use std::cmp::Ordering;

use xilem::WidgetView;
use xilem::core::Edit;
use xilem::view::flex_row;

use crate::ui::component::list::ListItem;

pub trait ListSorter
where
    Self: Default + Sized + 'static,
{
    type Item;

    fn enabled(&self) -> bool;
    fn view(&mut self) -> impl WidgetView<Edit<Self>> + use<Self>;
    fn sort(&self, a: &Self::Item, b: &Self::Item, score_a: f32, score_b: f32) -> Ordering;
}

#[derive(Default)]
pub struct NoSorter<T>(std::marker::PhantomData<T>);

impl<T> ListSorter for NoSorter<T>
where
    T: ListItem + Default,
{
    type Item = T;

    fn enabled(&self) -> bool {
        false
    }

    fn view(&mut self) -> impl WidgetView<Edit<Self>> + use<T> {
        flex_row(())
    }

    fn sort(&self, _a: &Self::Item, _b: &Self::Item, _score_a: f32, _score_b: f32) -> Ordering {
        Ordering::Equal
    }
}
