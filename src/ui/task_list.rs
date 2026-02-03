use std::cmp::Ordering;

use thiserror::Error;
use xilem::WidgetView;
use xilem::core::one_of::Either;
use xilem::core::{Edit, Read};
use xilem::palette::css::BLACK;
use xilem::style::Style;
use xilem::view::{
    FlexExt, MainAxisAlignment, button, checkbox, flex_col, flex_row, label, spinner, text_button,
    text_input, zstack,
};

use crate::Task;
use crate::core::ServerError;
use crate::database::{create_task, delete_task, get_tasks, update_task};
use crate::ui::component::Form;
use crate::ui::component::form::Submit;
use crate::ui::component::list::sorter::ListSorter;
use crate::ui::component::list::storage::Retryable;
use crate::ui::component::list::{
    ItemAction, ListFilter, ListItem, ListStorage, PendingItemOperation,
};
use crate::ui::theme::{DANGER_COLOR, SUCCESS_COLOR, SURFACE_BORDER_COLOR, SURFACE_COLOR};

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("description is required")]
    EmptyDescription,
}

#[derive(Debug, Default)]
pub struct CreateTaskForm {
    description: String,
    last_error: Option<TaskError>,
}

impl Form for CreateTaskForm {
    type Output = String;
    type Error = TaskError;

    fn last_error(&mut self) -> &mut Option<TaskError> {
        &mut self.last_error
    }

    fn view(&mut self) -> impl WidgetView<Edit<Self>, Submit> + use<> {
        let description = text_input(
            self.description.clone(),
            |state: &mut CreateTaskForm, input| {
                state.description = input;
                Submit::No
            },
        )
        .on_enter(|_, _| Submit::Yes)
        .placeholder("What needs to be done?");
        let add_button = text_button("Add task", |_| Submit::Yes);
        let error = self.error_view();
        flex_col((flex_row((description.flex(1.), add_button)), error))
            .padding(25.)
            .corner_radius(15.)
            .background_color(SURFACE_COLOR)
            .border(SURFACE_BORDER_COLOR, 1.)
    }

    fn validate(&mut self) -> Result<String, TaskError> {
        if self.description.is_empty() {
            return Err(TaskError::EmptyDescription);
        }
        Ok(std::mem::take(&mut self.description))
    }
}

#[derive(Debug, Default)]
pub struct UpdateTaskForm {
    description: String,
    done: bool,
    last_error: Option<TaskError>,
}

impl Form for UpdateTaskForm {
    type Output = (String, bool);
    type Error = TaskError;

    fn last_error(&mut self) -> &mut Option<TaskError> {
        &mut self.last_error
    }

    fn view(&mut self) -> impl WidgetView<Edit<Self>, Submit> + use<> {
        let description = text_input(self.description.clone(), |state: &mut Self, input| {
            state.description = input;
            Submit::No
        })
        .on_enter(|_, _| Submit::Yes);
        let ok_button = button(label("Ok").color(SUCCESS_COLOR), |_| Submit::Yes);
        let cancel_button = text_button("Cancel", |_| Submit::Cancel);
        let error = self.error_view();
        flex_col((
            flex_row((description.flex(1.), ok_button, cancel_button)),
            error,
        ))
        .padding(5.)
        .corner_radius(10.)
        .background_color(SURFACE_COLOR)
        .border(SURFACE_BORDER_COLOR, 1.)
    }

    fn validate(&mut self) -> Result<(String, bool), TaskError> {
        if self.description.is_empty() {
            return Err(TaskError::EmptyDescription);
        }
        Ok((
            std::mem::take(&mut self.description),
            std::mem::take(&mut self.done),
        ))
    }
}

impl From<Task> for UpdateTaskForm {
    fn from(value: Task) -> Self {
        Self {
            description: value.description.clone(),
            done: value.done,
            ..Default::default()
        }
    }
}

#[derive(Default, PartialEq, Clone, Copy)]
pub enum TaskFilter {
    All,
    #[default]
    Active,
    Completed,
}

impl ListFilter for TaskFilter {
    type Item = Task;

    fn view(&mut self) -> impl WidgetView<Edit<Self>> + use<> {
        let filter_task = |label, filter| {
            checkbox::<_, Edit<Self>, _>(label, *self == filter, move |state: &mut Self, _| {
                *state = filter
            })
        };
        flex_row((
            filter_task("All", Self::All),
            filter_task("Active", Self::Active),
            filter_task("Completed", Self::Completed),
        ))
        .main_axis_alignment(MainAxisAlignment::End)
    }
    fn filter(&self, task: &Task) -> (bool, f32) {
        let filter = match self {
            Self::All => true,
            Self::Active => !task.done,
            Self::Completed => task.done,
        };
        (filter, 0.)
    }
}

#[derive(Default)]
pub struct TaskSorter {
    reverse: bool,
}

impl ListSorter for TaskSorter {
    type Item = Task;

    fn enabled(&self) -> bool {
        true
    }

    fn view(&mut self) -> impl WidgetView<Edit<Self>> + use<> {
        let button = text_button(
            if self.reverse {
                "Descending"
            } else {
                "Ascending"
            },
            |state: &mut Self| state.reverse = !state.reverse,
        );
        flex_row(button).main_axis_alignment(MainAxisAlignment::End)
    }

    fn sort(&self, a: &Self::Item, b: &Self::Item, _score_a: f32, _score_bb: f32) -> Ordering {
        let ordering = a.id.cmp(&b.id);
        if self.reverse {
            return ordering.reverse();
        }
        ordering
    }
}

#[derive(Debug, Default)]
pub struct TaskStorage {
    last_error: Option<ServerError>,
}

impl Retryable for ServerError {
    fn should_retry(&self) -> bool {
        false
    }
}

impl ListStorage for TaskStorage {
    type Item = Task;
    type Error = ServerError;

    fn last_error(&mut self) -> &mut Option<ServerError> {
        &mut self.last_error
    }

    #[inline(always)]
    async fn fetch_all() -> Result<Vec<Task>, ServerError> {
        get_tasks().await
    }

    #[inline(always)]
    async fn create(description: String) -> Result<Task, ServerError> {
        create_task(description).await
    }

    #[inline(always)]
    async fn update(id: i64, (desc, done): (String, bool)) -> Result<Task, ServerError> {
        update_task(id, desc, done).await
    }

    #[inline(always)]
    async fn delete(id: i64) -> Result<i64, ServerError> {
        delete_task(id).await
    }
}

impl ListItem for Task {
    type Id = i64;
    type CreateForm = CreateTaskForm;
    type UpdateForm = UpdateTaskForm;
    type Filter = TaskFilter;
    type Sorter = TaskSorter;

    fn id(&self) -> i64 {
        self.id
    }

    fn view(
        &self,
        pending_item_operation: PendingItemOperation,
    ) -> impl WidgetView<Read<Self>, ItemAction<Self>> + use<> {
        let checkbox = checkbox(
            self.description.clone(),
            self.done,
            |state: &Self, checked| ItemAction::Update((state.description.clone(), checked)),
        );
        let edit_button = if matches!(pending_item_operation, PendingItemOperation::PendingUpdate) {
            Either::A(button(spinner(), |_| ItemAction::None))
        } else {
            Either::B(text_button("Edit", |_| ItemAction::Edit))
        };
        let delete_button = if matches!(pending_item_operation, PendingItemOperation::PendingDelete)
        {
            Either::A(button(spinner().color(DANGER_COLOR), |_| ItemAction::None))
        } else {
            Either::B(button(label("Delete").color(DANGER_COLOR), |_| {
                ItemAction::Delete
            }))
        };
        flex_row((checkbox.flex(1.), edit_button, delete_button))
            .padding(5.)
            .corner_radius(10.)
            .background_color(SURFACE_COLOR)
            .border(SURFACE_BORDER_COLOR, 1.)
    }

    fn pending_view(create_output: &String) -> impl WidgetView<Read<String>> + use<> {
        let checkbox = checkbox(create_output.clone(), false, |_, _| {});
        let edit_button = text_button("Edit", |_| {}).disabled(true);
        let delete_button = text_button("Delete", |_| {}).disabled(true);
        let pending_layer = flex_row((checkbox.flex(1.), edit_button, delete_button))
            .padding(5.)
            .corner_radius(10.)
            .background_color(SURFACE_COLOR);
        let spinner_layer = flex_row(spinner())
            .main_axis_alignment(MainAxisAlignment::Center)
            .padding(5.)
            .corner_radius(10.)
            .background_color(BLACK.with_alpha(0.25));
        zstack((pending_layer, spinner_layer))
    }
}
