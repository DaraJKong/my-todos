use thiserror::Error;
use xilem::WidgetView;
use xilem::core::{Edit, Read};
use xilem::style::Style;
use xilem::view::{FlexExt, button, checkbox, flex_col, flex_row, label, text_button, text_input};

use crate::Task;
use crate::database::{create_task, delete_task, get_tasks, update_task_done};
use crate::ui::component::Form;
use crate::ui::component::form::Submit;
use crate::ui::component::list::{ItemAction, ListItem, ListStorage};
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
    type Output = bool;
    type Error = TaskError;

    fn last_error(&mut self) -> &mut Option<TaskError> {
        &mut self.last_error
    }

    fn view(&mut self) -> impl WidgetView<Edit<Self>, Submit> + use<> {
        let checkbox = checkbox(
            self.description.clone(),
            self.done,
            |state: &mut UpdateTaskForm, checked| {
                state.done = checked;
                Submit::No
            },
        );
        let ok_button = button(label("Ok").color(SUCCESS_COLOR), |_| Submit::Yes);
        let cancel_button = text_button("Cancel", |_| Submit::Cancel);
        let error = self.error_view();
        flex_col((
            flex_row((checkbox.flex(1.), ok_button, cancel_button)),
            error,
        ))
        .padding(5.)
        .corner_radius(10.)
        .background_color(SURFACE_COLOR)
        .border(SURFACE_BORDER_COLOR, 1.)
    }

    fn validate(&mut self) -> Result<bool, TaskError> {
        Ok(std::mem::take(&mut self.done))
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

#[derive(Debug, Default)]
pub struct TaskStorage {
    last_error: Option<anyhow::Error>,
}

impl ListStorage for TaskStorage {
    type Item = Task;
    type Error = anyhow::Error;

    fn last_error(&mut self) -> &mut Option<anyhow::Error> {
        &mut self.last_error
    }

    #[inline(always)]
    async fn fetch_all() -> anyhow::Result<Vec<Task>> {
        get_tasks().await
    }

    #[inline(always)]
    async fn create(description: String) -> anyhow::Result<Task> {
        create_task(description).await
    }

    #[inline(always)]
    async fn update(id: i64, done: bool) -> anyhow::Result<Task> {
        update_task_done(id, done).await
    }

    #[inline(always)]
    async fn delete(id: i64) -> anyhow::Result<i64> {
        delete_task(id).await
    }
}

impl ListItem for Task {
    type Id = i64;
    type CreateForm = CreateTaskForm;
    type UpdateForm = UpdateTaskForm;

    fn id(&self) -> i64 {
        self.id
    }

    fn view(&self) -> impl WidgetView<Read<Self>, ItemAction<Self>> + use<> {
        let checkbox =
            checkbox(self.description.clone(), self.done, |_, checked| ItemAction::Update(checked)).flex(1.);
        let delete_button = button(label("Delete").color(DANGER_COLOR), |_| ItemAction::Delete);
        flex_row((checkbox, delete_button))
            .padding(5.)
            .corner_radius(10.)
            .background_color(SURFACE_COLOR)
            .border(SURFACE_BORDER_COLOR, 1.)
    }
}
