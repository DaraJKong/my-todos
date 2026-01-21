// On Windows platform, don't show a console when opening the app.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use todos::AppState;
use todos::ui::theme::apply_theme;
use xilem::masonry::theme::default_property_set;
use xilem::winit::error::EventLoopError;
use xilem::{EventLoop, EventLoopBuilder, Xilem};

fn run(event_loop: EventLoopBuilder) -> Result<(), EventLoopError> {
    let mut def_props = default_property_set();
    apply_theme(&mut def_props);

    let app = Xilem::new(AppState::default(), AppState::logic).with_default_properties(def_props);
    app.run_in(event_loop)
}

fn main() -> Result<(), EventLoopError> {
    run(EventLoop::with_user_event())
}
