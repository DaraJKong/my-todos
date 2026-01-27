pub mod filter;
pub mod sorter;
pub mod storage;

use uuid::Uuid;
use xilem::WidgetView;
use xilem::core::one_of::Either;
use xilem::core::{Edit, MessageProxy, Read, fork, lens, map_action, map_state};
use xilem::masonry::theme::BASIC_WIDGET_HEIGHT;
use xilem::style::Style;
use xilem::tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use xilem::view::{flex_col, spinner, worker};

use crate::ui::component::form::Submit;
pub use crate::ui::component::list::filter::ListFilter;
pub use crate::ui::component::list::sorter::ListSorter;
pub use crate::ui::component::list::storage::ListStorage;
use crate::ui::component::list::storage::Retryable;
use crate::ui::component::{ErrorView, Form};
use crate::ui::pending::Pending;

pub trait ListItem
where
    Self: Clone + std::fmt::Debug + Send + 'static,
{
    type Id: PartialEq + Copy + std::fmt::Debug + Send + Sync;
    type CreateForm: Form<Output: Clone + Send>;
    type UpdateForm: Form<Output: Clone + Send> + From<Self>;
    type Filter: ListFilter<Item = Self>;
    type Sorter: ListSorter<Item = Self>;

    fn id(&self) -> Self::Id;
    fn view(
        &self,
        pending_item_operation: PendingItemOperation,
    ) -> impl WidgetView<Read<Self>, ItemAction<Self>> + use<Self>;
    fn pending_view(
        create_output: &<Self::CreateForm as Form>::Output,
    ) -> impl WidgetView<Read<<Self::CreateForm as Form>::Output>> + use<Self> {
        let _ = create_output;
        spinner().height(BASIC_WIDGET_HEIGHT)
    }
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

#[derive(Default)]
pub enum PendingItemOperation {
    #[default]
    None,
    PendingUpdate,
    PendingDelete,
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

impl<T> Clone for ListRequest<T>
where
    T: ListItem,
{
    fn clone(&self) -> Self {
        match self {
            Self::FetchAll => Self::FetchAll,
            Self::Create(arg0) => Self::Create(arg0.clone()),
            Self::Update(arg0, arg1) => Self::Update(*arg0, arg1.clone()),
            Self::Delete(arg0) => Self::Delete(*arg0),
        }
    }
}

#[derive(Debug, Clone)]
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
    S: ListStorage,
{
    create_form: T::CreateForm,
    update_form: T::UpdateForm,
    filter: Option<T::Filter>,
    sorter: Option<T::Sorter>,
    editing: Option<T::Id>,
    items: Vec<T>,
    processed_items: Vec<(T, f32)>,
    sender: Option<UnboundedSender<Pending<ListRequest<T>>>>,
    pending_requests: Vec<Pending<ListRequest<T>>>,
    storage: S,
}

impl<T> ItemAction<T>
where
    T: ListItem,
{
    fn handle<S>(self, state: &mut AsyncList<T, S>, id: T::Id)
    where
        S: ListStorage<Item = T>,
    {
        match self {
            ItemAction::None => (),
            ItemAction::Edit => {
                if let Some(item) = state.get(id) {
                    state.update_form = T::UpdateForm::from(item.clone());
                    state.editing = Some(id);
                }
            }
            ItemAction::Update(update_output) => {
                state.send_request(ListRequest::Update(id, update_output));
            }
            ItemAction::Delete => {
                state.send_request(ListRequest::Delete(id));
            }
        }
    }
}

impl<T> Pending<ListRequest<T>>
where
    T: ListItem,
{
    async fn handle<S>(self, proxy: &MessageProxy<Pending<ListMessage<T, S>>>)
    where
        S: ListStorage<Item = T>,
    {
        let pending_message = self.map(async |list_request| match list_request {
            ListRequest::FetchAll => {
                let result = S::fetch_all().await;
                match result {
                    Ok(items) => ListMessage::FetchedAll(items),
                    Err(error) => ListMessage::Error(error),
                }
            }
            ListRequest::Create(create_output) => {
                let result = S::create(create_output).await;
                match result {
                    Ok(item) => ListMessage::Created(item),
                    Err(error) => ListMessage::Error(error),
                }
            }
            ListRequest::Update(id, update_output) => {
                let result = S::update(id, update_output).await;
                match result {
                    Ok(item) => ListMessage::Updated(id, item),
                    Err(error) => ListMessage::Error(error),
                }
            }
            ListRequest::Delete(id) => {
                let result = S::delete(id).await;
                match result {
                    Ok(id) => ListMessage::Deleted(id),
                    Err(error) => ListMessage::Error(error),
                }
            }
        });
        let _ = proxy.message(pending_message.await);
    }
}

impl<T, S> Pending<ListMessage<T, S>>
where
    T: ListItem,
    S: ListStorage<Item = T>,
{
    fn handle(self, state: &mut AsyncList<T, S>) {
        match self.data {
            ListMessage::FetchedAll(items) => state.items = items,
            ListMessage::Created(item) => {
                state.items.push(item);
            }
            ListMessage::Updated(id, new_item) => {
                if let Some(item) = state.get_mut(id) {
                    *item = new_item;
                }
            }
            ListMessage::Deleted(id) => {
                state.remove(id);
            }
            ListMessage::Error(error) => {
                if error.should_retry() {
                    state.retry_request(self.request_id);
                } else {
                    state.resolve_pending_request(self.request_id);
                }
                *state.storage.last_error() = Some(error);
                return;
            }
        }
        state.resolve_pending_request(self.request_id);
        *state.storage.last_error() = None;
    }
}

impl<T, S> AsyncList<T, S>
where
    T: ListItem,
    S: ListStorage<Item = T>,
{
    pub fn new(filter: bool, sorter: bool) -> Self {
        Self {
            create_form: T::CreateForm::default(),
            update_form: T::UpdateForm::default(),
            filter: filter.then_some(T::Filter::default()),
            sorter: sorter.then_some(T::Sorter::default()),
            editing: None,
            items: Vec::new(),
            processed_items: Vec::new(),
            pending_requests: Vec::new(),
            sender: None,
            storage: S::default(),
        }
    }

    fn filter(&self, item: &T) -> (bool, f32) {
        self.filter
            .as_ref()
            .map(|filter| filter.filter(item))
            .unwrap_or((true, 0.))
    }

    fn pending_item_operation(&self, id: T::Id) -> PendingItemOperation {
        self.pending_requests
            .iter()
            .find_map(|pending_request| match pending_request {
                Pending {
                    data: ListRequest::Update(pending_id, _),
                    ..
                } if *pending_id == id => Some(PendingItemOperation::PendingUpdate),
                Pending {
                    data: ListRequest::Delete(pending_id),
                    ..
                } if *pending_id == id => Some(PendingItemOperation::PendingDelete),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn send_request(&mut self, request: ListRequest<T>) {
        if let Some(sender) = &self.sender {
            let pending_request = Pending::new(request.clone());
            self.pending_requests
                .push(Pending::from((pending_request.request_id, request)));
            let _ = sender.send(pending_request);
        }
    }

    fn retry_request(&mut self, request_id: Uuid) {
        let pending_request = self
            .pending_requests
            .iter()
            .find(|pending_request| pending_request.request_id == request_id);
        if let (Some(sender), Some(pending_request)) = (&self.sender, pending_request) {
            let _ = sender.send(pending_request.clone().with_delay(0.5));
        }
    }

    fn resolve_pending_request(&mut self, request_id: Uuid) {
        if let Some(index) = self
            .pending_requests
            .iter()
            .enumerate()
            .find_map(|(i, pending)| (request_id == pending.request_id).then_some(i))
        {
            self.pending_requests.remove(index);
        }
    }

    fn get(&self, id: T::Id) -> Option<&T> {
        self.items.iter().find(|item| item.id() == id)
    }

    fn get_mut(&mut self, id: T::Id) -> Option<&mut T> {
        self.items.iter_mut().find(|item| item.id() == id)
    }

    fn remove(&mut self, id: T::Id) {
        if let Some(index) = self
            .items
            .iter()
            .enumerate()
            .find_map(|(i, item)| (item.id() == id).then_some(i))
        {
            self.items.remove(index);
        }
    }

    fn handle_create_submit(&mut self, submit: Submit) {
        match submit {
            Submit::No => (),
            Submit::Cancel => {
                self.create_form.reset();
            }
            Submit::Yes => {
                if let Some(output) = self.create_form.submit() {
                    self.send_request(ListRequest::Create(output));
                }
            }
        }
    }

    fn handle_update_submit(&mut self, id: T::Id, submit: Submit) {
        match submit {
            Submit::No => (),
            Submit::Cancel => {
                self.editing = None;
                self.update_form.reset();
            }
            Submit::Yes => {
                if let Some(output) = self.update_form.submit() {
                    self.editing = None;
                    self.send_request(ListRequest::Update(id, output));
                }
            }
        }
    }

    fn item_view(
        editing: bool,
        pending_item_operation: PendingItemOperation,
        id: T::Id,
        item: &T,
    ) -> impl WidgetView<Edit<Self>> + use<T, S> {
        if editing {
            Either::A(map_action(
                lens(
                    <T::UpdateForm as Form>::view,
                    move |state: &mut Self, ()| &mut state.update_form,
                ),
                move |state: &mut Self, submit| {
                    state.handle_update_submit(id, submit);
                },
            ))
        } else {
            Either::B(map_action(
                map_state(
                    item.view(pending_item_operation),
                    move |state: &mut Self, ()| state.get(id).unwrap(),
                ),
                move |state: &mut Self, action| {
                    action.handle(state, id);
                },
            ))
        }
    }

    fn process_items(&mut self) -> impl Iterator<Item = impl WidgetView<Edit<Self>> + use<T, S>> {
        self.processed_items = self
            .items
            .iter()
            .cloned()
            .filter_map(|item| match self.filter(&item) {
                (filter, score) if filter => Some((item, score)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if let Some(sorter) = &self.sorter {
            self.processed_items
                .sort_by(|(a, sa), (b, sb)| sorter.sort(a, b, *sa, *sb));
        }
        self.processed_items.iter().map(|(item, _)| {
            let id = item.id();
            let editing = self.editing == Some(id);
            let pending_item_operation = self.pending_item_operation(id);
            Self::item_view(editing, pending_item_operation, id, item)
        })
    }

    fn process_pending_items(
        &self,
    ) -> impl Iterator<Item = impl WidgetView<Edit<Self>> + use<T, S>> {
        self.pending_requests
            .iter()
            .enumerate()
            .filter_map(|(i, pending_request)| {
                matches!(pending_request.data, ListRequest::Create(_)).then_some(lens(
                    T::pending_view,
                    move |state: &mut Self, ()| match &state.pending_requests.get(i).unwrap().data {
                        ListRequest::Create(create_output) => create_output,
                        _ => unreachable!(),
                    },
                ))
            })
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
        let filter_line = self.filter.as_mut().map(|filter| {
            map_state(filter.view(), move |state: &mut Self, ()| {
                state.filter.as_mut().unwrap()
            })
        });
        let sorter_line = self.sorter.as_mut().map(|sorter| {
            map_state(sorter.view(), move |state: &mut Self, ()| {
                state.sorter.as_mut().unwrap()
            })
        });
        let items = self.process_items().collect::<Vec<_>>();
        let pending_items = self.process_pending_items().collect::<Vec<_>>();
        fork(
            flex_col((create_line, filter_line, sorter_line, items, pending_items)),
            worker(
                |proxy, mut rx: UnboundedReceiver<Pending<ListRequest<T>>>| async move {
                    while let Some(pending_request) = rx.recv().await {
                        pending_request.handle(&proxy).await;
                    }
                },
                |state: &mut Self, sender| {
                    state.sender = Some(sender);
                    state.send_request(ListRequest::FetchAll);
                },
                |state: &mut Self, pending_message: Pending<ListMessage<T, S>>| {
                    pending_message.handle(state);
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
