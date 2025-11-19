// On Windows platform, don't show a console when opening the app.
// #![windows_subsystem = "windows"]

use sqlx::{FromRow, SqlitePool};
use tokio::sync::mpsc::UnboundedSender;
use xilem::core::{fork, one_of::Either};
use xilem::masonry::properties::types::AsUnit;
use xilem::style::Style as _;
use xilem::view::{
    checkbox, flex_col, flex_row, sized_box, spinner, text_button, text_input, worker,
};
use xilem::winit::error::EventLoopError;
use xilem::{EventLoop, EventLoopBuilder, InsertNewline, WidgetView, WindowOptions, Xilem};

struct AppState {
    db_sender: Option<UnboundedSender<DbRequest>>,
    task_list: TaskList,
}

#[derive(Debug)]
enum DbRequest {
    FetchAllTodos,
    FetchTodo(i64),
    AddTodo(String, bool),
}

#[derive(Debug)]
enum DbMessage {
    AllTodosFetched(Vec<Task>),
    TodoFetched(Task),
    TodoAdded(i64),
}

#[derive(FromRow, Clone, Debug)]
struct Task {
    id: i64,
    description: String,
    done: bool,
}

#[derive(PartialEq, Eq, Copy, Clone, Default)]
enum Filter {
    #[default]
    All,
    Active,
    Completed,
}

enum TaskStatus {
    Pending(i64),
    Available(Task),
}

#[derive(Default)]
struct TaskList {
    next_task: String,
    filter: Filter,
    tasks: Vec<TaskStatus>,
}

impl TaskList {
    fn new(tasks: Vec<Task>) -> Self {
        Self {
            tasks: tasks
                .iter()
                .map(|task| TaskStatus::Available(task.clone()))
                .collect(),
            ..Default::default()
        }
    }

    fn add_task(&mut self, sender: Option<&UnboundedSender<DbRequest>>) {
        if let Some(sender) = sender {
            if !self.next_task.is_empty() {
                sender
                    .send(DbRequest::AddTodo(
                        std::mem::take(&mut self.next_task),
                        false,
                    ))
                    .unwrap();
            }
        }
    }

    fn add_pending(&mut self, id: i64, sender: Option<&UnboundedSender<DbRequest>>) {
        self.tasks.push(TaskStatus::Pending(id));
        if let Some(sender) = sender {
            sender.send(DbRequest::FetchTodo(id)).unwrap();
        }
    }

    fn update_pending(&mut self, id: i64, new_task: Task) {
        if let Some(task) = self.tasks.iter_mut().find(|task| {
            if let TaskStatus::Pending(pending_id) = task {
                return id == *pending_id;
            };
            false
        }) {
            *task = TaskStatus::Available(new_task);
        }
    }
}

fn app_logic(state: &mut AppState) -> impl WidgetView<AppState> + use<> {
    let input_box = text_input(
        state.task_list.next_task.clone(),
        |state: &mut AppState, new_value| {
            state.task_list.next_task = new_value;
        },
    )
    .placeholder("What needs to be done?")
    .insert_newline(InsertNewline::OnShiftEnter)
    .on_enter(|state: &mut AppState, _| {
        state.task_list.add_task(state.db_sender.as_ref());
    });

    let first_line = flex_col((
        input_box,
        text_button("Add task".to_string(), |state: &mut AppState| {
            state.task_list.add_task(state.db_sender.as_ref());
        }),
    ));

    let tasks = state
        .task_list
        .tasks
        .iter()
        .enumerate()
        .filter_map(|(i, task_state)| match task_state {
            TaskStatus::Pending(_id) => Some(Either::A(flex_row(
                sized_box(spinner()).height(40.px()).width(40.px()),
            ))),
            TaskStatus::Available(task) => {
                if (state.task_list.filter == Filter::Active && task.done)
                    || (state.task_list.filter == Filter::Completed && !task.done)
                {
                    None
                } else {
                    let checkbox = checkbox(
                        task.description.clone(),
                        task.done,
                        move |state: &mut AppState, checked| {
                            if let Some(TaskStatus::Available(task)) =
                                state.task_list.tasks.get_mut(i)
                            {
                                task.done = checked;
                            }
                        },
                    );
                    let delete_button = text_button("Delete", move |state: &mut AppState| {
                        state.task_list.tasks.remove(i);
                    });
                    Some(Either::B(flex_row((checkbox, delete_button))))
                }
            }
        })
        .collect::<Vec<_>>();

    let filter_tasks = |label, filter| {
        // TODO: replace with combo-buttons
        checkbox(
            label,
            state.task_list.filter == filter,
            move |state: &mut AppState, _| state.task_list.filter = filter,
        )
    };
    let has_tasks = !state.task_list.tasks.is_empty();
    let footer = has_tasks.then(|| {
        flex_row((
            filter_tasks("All", Filter::All),
            filter_tasks("Active", Filter::Active),
            filter_tasks("Completed", Filter::Completed),
        ))
    });

    fork(
        flex_col((first_line, tasks, footer)).padding(50.0),
        worker(
            |proxy, mut rx| async move {
                let database_url = "sqlite://db/Todos.db";
                let pool = SqlitePool::connect(&database_url)
                    .await
                    .expect("Failed to connect to SQLite");

                while let Some(req) = rx.recv().await {
                    let proxy = proxy.clone();
                    let pool = pool.clone();
                    tokio::task::spawn(async move {
                        match req {
                            DbRequest::FetchAllTodos => {
                                let result = fetch_all_todos(&pool).await;
                                match result {
                                    Ok(tasks) => {
                                        drop(proxy.message(DbMessage::AllTodosFetched(tasks)))
                                    }
                                    Err(err) => {
                                        println!("Fetching all todos in database failed: {err:?}");
                                    }
                                }
                            }
                            DbRequest::FetchTodo(id) => {
                                let result = fetch_todo(&pool, id).await;
                                match result {
                                    Ok(task) => drop(proxy.message(DbMessage::TodoFetched(task))),
                                    Err(err) => {
                                        println!("Fetching todo in database failed: {err:?}");
                                    }
                                }
                            }
                            DbRequest::AddTodo(desc, done) => {
                                let result = add_todo(&pool, desc, done).await;
                                match result {
                                    Ok(id) => drop(proxy.message(DbMessage::TodoAdded(id))),
                                    Err(err) => {
                                        println!("Adding todo in database failed: {err:?}");
                                    }
                                }
                            }
                        }
                    });
                }
            },
            |state: &mut AppState, sender| {
                state.db_sender = Some(sender);
                if let Some(sender) = &state.db_sender {
                    sender.send(DbRequest::FetchAllTodos);
                }
            },
            |state: &mut AppState, msg: DbMessage| match msg {
                DbMessage::AllTodosFetched(tasks) => {
                    state.task_list = TaskList::new(tasks);
                }
                DbMessage::TodoFetched(task) => {
                    state.task_list.update_pending(task.id, task);
                }
                DbMessage::TodoAdded(id) => {
                    state.task_list.add_pending(id, state.db_sender.as_ref());
                }
            },
        ),
    )
}

async fn fetch_all_todos(pool: &SqlitePool) -> anyhow::Result<Vec<Task>> {
    let tasks = sqlx::query_as::<_, Task>("SELECT id, description, done FROM todos")
        .fetch_all(pool)
        .await?;
    Ok(tasks)
}

async fn fetch_todo(pool: &SqlitePool, id: i64) -> anyhow::Result<Task> {
    let task = sqlx::query_as::<_, Task>("SELECT id, description, done FROM todos WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    std::thread::sleep(std::time::Duration::from_millis(1000));
    Ok(task)
}

async fn add_todo(pool: &SqlitePool, desc: String, done: bool) -> anyhow::Result<i64> {
    let mut conn = pool.acquire().await?;
    let id = sqlx::query("INSERT INTO todos (description, done) VALUES (?, ?)")
        .bind(desc)
        .bind(done)
        .execute(&mut *conn)
        .await?
        .last_insert_rowid();
    Ok(id)
}

fn run(event_loop: EventLoopBuilder) -> Result<(), EventLoopError> {
    let data = AppState {
        db_sender: None,
        task_list: TaskList::default(),
    };

    let app = Xilem::new_simple(data, app_logic, WindowOptions::new("Todos"));
    app.run_in(event_loop)
}

fn main() -> Result<(), EventLoopError> {
    run(EventLoop::with_user_event())
}
