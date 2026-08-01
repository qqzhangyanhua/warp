use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

pub(crate) const PERSONAL_MEMORY_RECORD_LIMIT: i64 = 5_000;
pub(crate) const PERSONAL_MEMORY_QUERY_LIMIT: usize = 5;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PersonalMemoryIndexState {
    Unconfigured,
    Pending,
    Ready,
    Unavailable,
}

impl PersonalMemoryIndexState {
    pub(crate) fn as_database_value(self) -> &'static str {
        match self {
            Self::Unconfigured => "unconfigured",
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Unavailable => "unavailable",
        }
    }

    pub(crate) fn from_database_value(value: &str) -> Option<Self> {
        match value {
            "unconfigured" => Some(Self::Unconfigured),
            "pending" => Some(Self::Pending),
            "ready" => Some(Self::Ready),
            "unavailable" => Some(Self::Unavailable),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct PersonalMemoryRecord {
    pub(crate) record_id: String,
    pub(crate) fact_text: String,
    pub(crate) value_text: String,
    pub(crate) topic: String,
    pub(crate) normalized_topic: String,
    pub(crate) labels: Vec<String>,
    pub(crate) is_default: bool,
    pub(crate) index_state: PersonalMemoryIndexState,
    pub(crate) created_at: NaiveDateTime,
    pub(crate) updated_at: NaiveDateTime,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PersonalMemoryVector {
    pub(crate) record_id: String,
    pub(crate) values: Vec<f32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NewPersonalMemoryRecord {
    pub(crate) record_id: String,
    pub(crate) fact_text: String,
    pub(crate) value_text: String,
    pub(crate) topic: String,
    pub(crate) normalized_topic: String,
    pub(crate) labels: Vec<String>,
    pub(crate) is_default: bool,
    pub(crate) index_state: PersonalMemoryIndexState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CreatePersonalMemoryResult {
    Created(PersonalMemoryRecord),
    AlreadyRemembered(PersonalMemoryRecord),
}

impl CreatePersonalMemoryResult {
    pub(crate) fn record(&self) -> &PersonalMemoryRecord {
        match self {
            Self::Created(record) | Self::AlreadyRemembered(record) => record,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum QueryPersonalMemoryResult {
    Matches(Vec<PersonalMemoryRecord>),
    NoMatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CreatePersonalMemoryInput {
    pub(crate) fact_text: String,
    pub(crate) value_text: String,
    pub(crate) topic: String,
    pub(crate) labels: Vec<String>,
    pub(crate) is_default: bool,
}

impl CreatePersonalMemoryInput {
    pub(crate) fn exact(fact_text: String, value_text: String, topic: String) -> Self {
        Self {
            fact_text,
            value_text,
            topic,
            labels: Vec::new(),
            is_default: false,
        }
    }
}
