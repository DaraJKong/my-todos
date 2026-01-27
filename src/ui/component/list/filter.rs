use xilem::WidgetView;
use xilem::core::Edit;
use xilem::view::flex_row;

use crate::ui::component::list::ListItem;

pub trait ListFilter
where
    Self: Default + Sized + 'static,
{
    type Item;

    fn view(&mut self) -> impl WidgetView<Edit<Self>> + use<Self>;
    /// This function should return `(filter, score)` where a true `filter` value means the item
    /// should be included and where `score` is the matching score used in sorting. `score` should
    /// be between `0.0` and `1.0`. To disable filtering completely, please return `(true, 0.0)`
    /// so that `ListSorter` can ignore the `score` value when sorting.
    fn filter(&self, item: &Self::Item) -> (bool, f32);
}

#[derive(Default)]
pub struct NoFilter<T>(std::marker::PhantomData<T>);

impl<T> ListFilter for NoFilter<T>
where
    T: ListItem + Default,
{
    type Item = T;

    fn view(&mut self) -> impl WidgetView<Edit<Self>> + use<T> {
        flex_row(())
    }

    fn filter(&self, _item: &Self::Item) -> (bool, f32) {
        (true, 0.)
    }
}
