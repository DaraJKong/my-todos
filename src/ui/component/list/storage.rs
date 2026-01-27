use crate::ui::component::list::ListItem;
use crate::ui::component::{ErrorView, Form};

pub trait ListStorage
where
    Self: Default + std::fmt::Debug + 'static,
{
    type Item: ListItem;
    type Error: ErrorView + Retryable + std::fmt::Debug + Send;

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

pub trait Retryable {
    fn should_retry(&self) -> bool;
}
