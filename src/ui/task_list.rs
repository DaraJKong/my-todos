use std::cmp::Ordering;

use thiserror::Error;
use xilem::WidgetView;
use xilem::core::one_of::Either;
use xilem::core::{Edit, Read};
use xilem::palette::css::BLACK;
use xilem::style::Style;
use xilem::view::{
    FlexExt, MainAxisAlignment, button, checkbox, flex_col, flex_row, label, prose, spinner,
    text_button, text_input, zstack,
};

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
use crate::{Priority, Status, Task};

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
    status: Status,
    priority: Priority,
    last_error: Option<TaskError>,
}

impl Form for UpdateTaskForm {
    type Output = (String, Status, Priority);
    type Error = TaskError;

    fn last_error(&mut self) -> &mut Option<TaskError> {
        &mut self.last_error
    }

    fn view(&mut self) -> impl WidgetView<Edit<Self>, Submit> + use<> {
        let status = text_button(self.status.to_string(), |state: &mut Self| {
            state.status = state.status.next();
            Submit::No
        })
        .background_color(self.status.color());
        let description = text_input(self.description.clone(), |state: &mut Self, input| {
            state.description = input;
            Submit::No
        })
        .on_enter(|_, _| Submit::Yes);
        let priority = button(
            label(self.priority.to_string()).color(self.priority.text_color()),
            |state: &mut Self| {
                state.priority = state.priority.next();
                Submit::No
            },
        );
        let ok_button = button(label("Ok").color(SUCCESS_COLOR), |_| Submit::Yes);
        let cancel_button = text_button("Cancel", |_| Submit::Cancel);
        let error = self.error_view();
        flex_col((
            flex_row((
                status,
                description.flex(1.),
                priority,
                ok_button,
                cancel_button,
            )),
            error,
        ))
        .padding(5.)
        .corner_radius(10.)
        .background_color(SURFACE_COLOR)
        .border(self.priority.color(), 1.)
    }

    fn validate(&mut self) -> Result<(String, Status, Priority), TaskError> {
        if self.description.is_empty() {
            return Err(TaskError::EmptyDescription);
        }
        Ok((
            std::mem::take(&mut self.description),
            self.status,
            self.priority,
        ))
    }
}

impl From<Task> for UpdateTaskForm {
    fn from(value: Task) -> Self {
        Self {
            description: value.description.clone(),
            status: value.status,
            priority: value.priority,
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
            Self::Active => !matches!(task.status, Status::Done),
            Self::Completed => matches!(task.status, Status::Done),
        };
        (filter, 0.)
    }
}

#[derive(Default)]
pub enum TaskSorter {
    #[default]
    StatusFirst,
    PriorityFirst,
}

impl ListSorter for TaskSorter {
    type Item = Task;

    fn enabled(&self) -> bool {
        true
    }

    fn view(&mut self) -> impl WidgetView<Edit<Self>> + use<> {
        let button = text_button(
            match self {
                TaskSorter::StatusFirst => "Status first",
                TaskSorter::PriorityFirst => "Priority first",
            },
            |state: &mut Self| match state {
                TaskSorter::StatusFirst => *state = TaskSorter::PriorityFirst,
                TaskSorter::PriorityFirst => *state = TaskSorter::StatusFirst,
            },
        );
        flex_row(button).main_axis_alignment(MainAxisAlignment::End)
    }

    fn sort(&self, a: &Self::Item, b: &Self::Item, _score_a: f32, _score_b: f32) -> Ordering {
        let status_ordering = (a.status as i32).cmp(&(b.status as i32));
        let priority_ordering = (b.priority as i32).cmp(&(a.priority as i32));
        let id_ordering = b.id.cmp(&a.id);
        match self {
            TaskSorter::StatusFirst => status_ordering.then(priority_ordering),
            TaskSorter::PriorityFirst => priority_ordering.then(status_ordering),
        }
        .then(id_ordering)
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
    async fn update(
        id: i64,
        (desc, status, priority): (String, Status, Priority),
    ) -> Result<Task, ServerError> {
        update_task(id, desc, status, priority).await
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
        let status = text_button(self.status.to_string(), |state: &Self| {
            ItemAction::Update((
                state.description.clone(),
                state.status.next(),
                state.priority,
            ))
        })
        .background_color(self.status.color());
        let description = prose(self.description.clone());
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
        flex_row((status, description.flex(1.), edit_button, delete_button))
            .padding(5.)
            .corner_radius(10.)
            .background_color(SURFACE_COLOR)
            .border(self.priority.color(), 1.)
    }

    fn pending_view(create_output: &String) -> impl WidgetView<Read<String>> + use<> {
        let status = text_button(Status::ToDo.to_string(), |_| {}).disabled(true);
        let description = prose(create_output.clone());
        let edit_button = text_button("Edit", |_| {}).disabled(true);
        let delete_button = text_button("Delete", |_| {}).disabled(true);
        let pending_layer = flex_row((status, description.flex(1.), edit_button, delete_button))
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
