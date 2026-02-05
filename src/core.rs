use std::fmt;

use sqlx::{Error as SqlxError, FromRow, Type};
use thiserror::Error;
use xilem::Color;
use xilem::palette::css::{ORANGE_RED, DODGER_BLUE, GOLD, LIME_GREEN, RED, WHITE};

use crate::ui::theme::SURFACE_BORDER_COLOR;

#[derive(Default, Type, Copy, Clone, Debug)]
#[repr(i32)]
#[non_exhaustive]
pub enum Status {
    #[default]
    ToDo,
    InProgress,
    Done,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Status::ToDo => write!(f, "To Do"),
            Status::InProgress => write!(f, "In Progress"),
            Status::Done => write!(f, "Done"),
        }
    }
}

impl Status {
    pub fn next(&self) -> Self {
        match self {
            Status::ToDo => Status::InProgress,
            Status::InProgress => Status::Done,
            Status::Done => Status::ToDo,
        }
    }

    #[inline]
    pub fn color(&self) -> Color {
        match self {
            Status::ToDo => DODGER_BLUE,
            Status::InProgress => ORANGE_RED,
            Status::Done => LIME_GREEN,
        }
    }
}

#[derive(Default, Type, Copy, Clone, Debug)]
#[repr(i32)]
#[non_exhaustive]
pub enum Priority {
    #[default]
    Low,
    Medium,
    High,
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Priority::Low => write!(f, "Low"),
            Priority::Medium => write!(f, "Medium"),
            Priority::High => write!(f, "High"),
        }
    }
}

impl Priority {
    pub fn next(&self) -> Self {
        match self {
            Priority::Low => Priority::Medium,
            Priority::Medium => Priority::High,
            Priority::High => Priority::Low,
        }
    }

    #[inline]
    pub fn color(&self) -> Color {
        match self {
            Priority::Low => SURFACE_BORDER_COLOR,
            Priority::Medium => GOLD,
            Priority::High => RED,
        }
    }

    #[inline]
    pub fn text_color(&self) -> Color {
        match self {
            Priority::Low => WHITE,
            Priority::Medium => GOLD,
            Priority::High => RED,
        }
    }
}

#[derive(Default, FromRow, Clone, Debug)]
pub struct Task {
    pub id: i64,
    pub description: String,
    pub status: Status,
    pub priority: Priority,
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("received a database error: {0}")]
    Database(SqlxError),
}

impl From<SqlxError> for ServerError {
    fn from(value: SqlxError) -> Self {
        Self::Database(value)
    }
}
