pub mod task_item;

use xilem::WidgetView;
use xilem::core::one_of::Either;
use xilem::core::{Edit, MessageProxy, Read, fork, lens, map_action, map_state};
use xilem::masonry::layout::Dim;
use xilem::style::Style;
use xilem::tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use xilem::view::{flex_col, portal, worker};

use crate::ui::component::form::Submit;
use crate::ui::component::{ErrorView, Form};

pub trait ListStorage
where
    Self: Default + std::fmt::Debug + 'static,
{
    type Item: ListItem;
    type Error: ErrorView + std::fmt::Debug + Send;

    fn last_error(&mut self) -> &mut Option<Self::Error>;

    fn fetch_all() -> impl Future<Output = Result<Vec<Self::Item>, Self::Error>> + Send;
    fn create(
        create_form: <<Self::Item as ListItem>::CreateForm as Form>::Output,
    ) -> impl Future<Output = Result<Self::Item, Self::Error>> + Send;
    fn update(
        id: <Self::Item as ListItem>::Id,
        update_form: <<Self::Item as ListItem>::UpdateForm as Form>::Output,
    ) -> impl Future<Output = Result<Self::Item, Self::Error>> + Send;
    fn delete(
        id: <Self::Item as ListItem>::Id,
    ) -> impl Future<Output = Result<<Self::Item as ListItem>::Id, Self::Error>> + Send;
}

pub trait ListFilter
where
    Self: Sized + 'static,
{
    type Item: ListItem;

    fn view(&mut self) -> impl WidgetView<Edit<Self>> + use<Self>;
    fn filter(&self, item: &Self::Item) -> bool;
}

pub trait ListItem
where
    Self: Sized + Clone + std::fmt::Debug + Send + 'static,
{
    /// Type `Id` should be a unique identifier.
    type Id: PartialEq + Copy + std::fmt::Debug + Send + Sync;
    type CreateForm: Form<Output: Send>;
    type UpdateForm: Form<Output: Send> + From<Self>;
    type Filter: ListFilter<Item = Self>;

    fn id(&self) -> Self::Id;
    fn view(&self) -> impl WidgetView<Read<Self>, ItemAction<Self>> + use<Self>;
}

pub enum ItemAction<T>
where
    T: ListItem,
{
    None,
    Edit,
    Update(<T::UpdateForm as Form>::Output),
    Delete,
}

pub enum ListRequest<T>
where
    T: ListItem,
{
    FetchAll,
    Create(<T::CreateForm as Form>::Output),
    Update(T::Id, <T::UpdateForm as Form>::Output),
    Delete(T::Id),
}

#[derive(Debug)]
pub enum ListMessage<T, S>
where
    T: ListItem,
    S: ListStorage,
{
    FetchedAll(Vec<T>),
    Created(T),
    Updated(T::Id, T),
    Deleted(T::Id),
    Error(S::Error),
}

#[derive(Default)]
pub struct AsyncList<T, S>
where
    T: ListItem,
    S: ListStorage<Item = T>,
{
    create_form: T::CreateForm,
    update_form: T::UpdateForm,
    filter: T::Filter,
    editing: Option<T::Id>,
    items: Vec<T>,
    sender: Option<UnboundedSender<ListRequest<T>>>,
    #[allow(dead_code)]
    storage: S,
}

impl<T> ItemAction<T>
where
    T: ListItem,
{
    fn handle<S>(self, state: &mut AsyncList<T, S>, index: usize, id: T::Id)
    where
        S: ListStorage<Item = T>,
    {
        match self {
            ItemAction::Edit => {
                if let Some(item) = state.items.get(index) {
                    state.update_form = T::UpdateForm::from(item.clone());
                    state.editing = Some(id);
                }
            }
            ItemAction::Update(update_output) => {
                state.sender_send(ListRequest::Update(id, update_output));
            }
            ItemAction::Delete => {
                state.sender_send(ListRequest::Delete(id));
            }
            _ => (),
        }
    }
}

impl<T> ListRequest<T>
where
    T: ListItem,
{
    async fn handle<S>(self, proxy: &MessageProxy<ListMessage<T, S>>)
    where
        S: ListStorage<Item = T>,
    {
        match self {
            ListRequest::FetchAll => {
                let result = S::fetch_all().await;
                match result {
                    Ok(items) => {
                        let _ = proxy.message(ListMessage::FetchedAll(items));
                    }
                    Err(error) => {
                        let _ = proxy.message(ListMessage::Error(error));
                    }
                }
            }
            ListRequest::Create(create_output) => {
                let result = S::create(create_output).await;
                match result {
                    Ok(item) => {
                        let _ = proxy.message(ListMessage::Created(item));
                    }
                    Err(error) => {
                        let _ = proxy.message(ListMessage::Error(error));
                    }
                }
            }
            ListRequest::Update(id, update_output) => {
                let result = S::update(id, update_output).await;
                match result {
                    Ok(item) => {
                        let _ = proxy.message(ListMessage::Updated(id, item));
                    }
                    Err(error) => {
                        let _ = proxy.message(ListMessage::Error(error));
                    }
                }
            }
            ListRequest::Delete(id) => {
                let result = S::delete(id).await;
                match result {
                    Ok(id) => {
                        let _ = proxy.message(ListMessage::Deleted(id));
                    }
                    Err(error) => {
                        let _ = proxy.message(ListMessage::Error(error));
                    }
                }
            }
        }
    }
}

impl<T, S> ListMessage<T, S>
where
    T: ListItem,
    S: ListStorage<Item = T>,
{
    fn handle(self, state: &mut AsyncList<T, S>) {
        match self {
            ListMessage::FetchedAll(items) => state.items = items,
            ListMessage::Created(item) => {
                state.update_form.reset();
                state.editing = None;
                state.items.push(item);
            }
            ListMessage::Updated(id, new_item) => {
                let index = state.index_of(id);
                let item_mut = index.and_then(|i| state.items.get_mut(i));
                if let Some(item) = item_mut {
                    *item = new_item;
                    state.editing = None;
                }
            }
            ListMessage::Deleted(id) => {
                if let Some(index) = state.index_of(id) {
                    state.update_form.reset();
                    state.editing = None;
                    state.items.remove(index);
                }
            }
            ListMessage::Error(error) => {
                *state.storage.last_error() = Some(error);
                return;
            }
        }
        *state.storage.last_error() = None;
    }
}

impl<T, S> AsyncList<T, S>
where
    T: ListItem,
    S: ListStorage<Item = T>,
{
    fn sender_send(&self, request: ListRequest<T>) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(request);
        }
    }

    fn index_of(&mut self, id: T::Id) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .find_map(|(i, item)| (item.id() == id).then_some(i))
    }

    fn handle_create_submit(&mut self, submit: Submit) {
        match submit {
            Submit::No => {
                return;
            }
            Submit::Cancel => {
                self.create_form.reset();
                return;
            }
            _ => (),
        }
        if let Some(output) = self.create_form.submit() {
            self.sender_send(ListRequest::Create(output));
        }
    }

    fn handle_update_submit(&mut self, index: usize, submit: Submit) {
        match submit {
            Submit::No => {
                return;
            }
            Submit::Cancel => {
                self.editing = None;
                self.update_form.reset();
                return;
            }
            _ => (),
        }
        let item = self.items.get(index);
        let output = self.update_form.submit();
        if let (Some(item), Some(output)) = (item, output) {
            self.sender_send(ListRequest::Update(item.id(), output));
        }
    }

    fn item_view(
        filter: bool,
        editing: bool,
        index: usize,
        id: T::Id,
    ) -> Option<impl WidgetView<Edit<Self>> + use<T, S>> {
        if !filter {
            return None;
        }
        let either = if editing {
            Either::A(map_action(
                lens(
                    <T::UpdateForm as Form>::view,
                    move |state: &mut Self, ()| &mut state.update_form,
                ),
                move |state: &mut Self, submit| {
                    state.handle_update_submit(index, submit);
                },
            ))
        } else {
            Either::B(map_action(
                lens(T::view, move |state: &mut Self, ()| {
                    state.items.get(index).unwrap()
                }),
                move |state: &mut Self, action| {
                    action.handle(state, index, id);
                },
            ))
        };
        Some(either)
    }

    pub fn view(&mut self) -> impl WidgetView<Edit<Self>> + use<T, S> {
        let create_line = map_action(
            lens(
                <T::CreateForm as Form>::view,
                move |state: &mut Self, ()| &mut state.create_form,
            ),
            |state: &mut Self, submit| {
                state.handle_create_submit(submit);
            },
        );
        let filter_line = lens(
            <T::Filter as ListFilter>::view,
            move |state: &mut Self, ()| &mut state.filter,
        );
        let items = self
            .items
            .iter_mut()
            .enumerate()
            .map(|(i, item)| {
                let id = item.id();
                let filter = self.filter.filter(item);
                let editing = self.editing == Some(id);
                Self::item_view(filter, editing, i, id)
            })
            .collect::<Vec<_>>();
        fork(
            portal(
                flex_col((create_line, filter_line, items))
                    .width(Dim::Stretch)
                    .padding(15.),
            ),
            worker(
                |proxy, mut rx: UnboundedReceiver<ListRequest<T>>| async move {
                    while let Some(request) = rx.recv().await {
                        request.handle::<S>(&proxy).await;
                    }
                },
                |state: &mut Self, sender| {
                    state.sender = Some(sender);
                    state.sender_send(ListRequest::FetchAll);
                },
                |state: &mut Self, message: ListMessage<T, S>| {
                    message.handle(state);
                },
            ),
        )
    }

    pub fn error_view(&mut self) -> Option<impl WidgetView<Edit<Self>> + use<T, S>> {
        self.storage.last_error().as_ref().map(|error| {
            map_state(error.view(), move |state: &mut Self, ()| {
                state.storage.last_error().as_ref().unwrap()
            })
        })
    }
}
