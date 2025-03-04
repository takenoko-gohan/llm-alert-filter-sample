use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Formatter;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct FeedbackId(Uuid);

impl fmt::Display for FeedbackId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FeedbackId {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Timestamp(i64);

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<Timestamp> for DateTime<Utc> {
    type Error = Box<dyn std::error::Error>;

    fn try_from(value: Timestamp) -> Result<Self, Self::Error> {
        Ok(DateTime::from_timestamp(value.0, 0).ok_or("Invalid timestamp")?)
    }
}

impl Timestamp {
    pub(crate) fn new() -> Self {
        Self(chrono::Utc::now().timestamp())
    }
}
