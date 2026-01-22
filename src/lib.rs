pub mod ui;

use xilem::core::map_state;
use xilem::masonry::layout::AsUnit;
use xilem::style::Style as _;
use xilem::view::{FlexExt, MainAxisAlignment, flex_col, flex_row, sized_box};
use xilem::{WindowId, WindowView, window};

use crate::core::Task;
use crate::ui::component::AsyncList;
use crate::ui::component::list::task_item::TaskStorage;
use crate::ui::theme::BACKGROUND_COLOR;

pub mod core;
pub mod database;

enum TaskStatus {
    Pending(i64),
    Available(Task),
}

pub struct AppState {
    running: bool,
    main_window_id: WindowId,
    task_list: AsyncList<Task, TaskStorage>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            running: true,
            main_window_id: WindowId::next(),
            task_list: Default::default(),
        }
    }
}

impl xilem::AppState for AppState {
    fn keep_running(&self) -> bool {
        self.running
    }
}

impl AppState {
    pub fn logic(&mut self) -> impl Iterator<Item = WindowView<AppState>> + use<> {
        let task_list = flex_row(sized_box(self.task_list.view()).width(1000.px()))
            .main_axis_alignment(MainAxisAlignment::Center)
            .flex(1.);
        let error = self.task_list.error_view().map(|error_view| {
            flex_row(error_view)
                .main_axis_alignment(MainAxisAlignment::Center)
                .padding(15.)
        });
        let content = map_state(
            flex_col((task_list, error)).gap(0.px()),
            |state: &mut AppState, ()| &mut state.task_list,
        );
        std::iter::once(
            window(self.main_window_id, "Todos", content)
                .with_options(|options| {
                    options.on_close(|state: &mut AppState| state.running = false)
                })
                .with_base_color(BACKGROUND_COLOR),
        )
    }
}
