use std::cmp::Ordering;
use std::sync::mpsc::SyncSender;

use futures::channel::oneshot;
use uuid::Uuid;

mod capability;
mod types;

pub(crate) use capability::MemoryCapability;
pub(crate) use types::{
    CreatePersonalMemoryInput, CreatePersonalMemoryResult, NewPersonalMemoryRecord,
    PersonalMemoryIndexState, PersonalMemoryRecord,
    QueryPersonalMemoryResult, PERSONAL_MEMORY_QUERY_LIMIT, PERSONAL_MEMORY_RECORD_LIMIT,
};

use crate::persistence::{
    CreatePersonalMemory, ListPersonalMemories, ModelEvent, PersonalMemoryPersistenceError,
};

#[derive(Clone)]
pub(crate) struct PersonalMemoryService {
    persistence: SyncSender<ModelEvent>,
}

impl PersonalMemoryService {
    pub(crate) fn new(persistence: SyncSender<ModelEvent>) -> Self {
        Self { persistence }
    }

    pub(crate) async fn create(
        &self,
        input: CreatePersonalMemoryInput,
    ) -> Result<CreatePersonalMemoryResult, PersonalMemoryError> {
        validate_create_input(&input)?;
        let record = NewPersonalMemoryRecord {
            record_id: Uuid::new_v4().to_string(),
            normalized_topic: normalize_search_text(&input.topic),
            fact_text: input.fact_text,
            value_text: input.value_text,
            topic: input.topic,
            labels: input.labels,
            is_default: input.is_default,
            index_state: PersonalMemoryIndexState::Unconfigured,
        };
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.persistence
            .send(ModelEvent::CreatePersonalMemory(CreatePersonalMemory {
                record,
                acknowledgement,
            }))
            .map_err(|_| PersonalMemoryError::PersistenceUnavailable)?;
        let result = acknowledged
            .await
            .map_err(|_| PersonalMemoryError::PersistenceAcknowledgementDropped)?
            .map_err(PersonalMemoryError::from)?;
        Ok(result)
    }

    pub(crate) async fn query(
        &self,
        query: &str,
    ) -> Result<QueryPersonalMemoryResult, PersonalMemoryError> {
        if query.trim().is_empty() {
            return Err(PersonalMemoryError::InvalidQuery);
        }
        let records = self.list().await?;
        let normalized_query = normalize_search_text(query);
        let mut exact_records = records
            .iter()
            .filter(|record| match_score(record, &normalized_query).is_some())
            .cloned()
            .collect::<Vec<_>>();
        exact_records.sort_by(|left, right| {
            match_score(left, &normalized_query)
                .cmp(&match_score(right, &normalized_query))
                .then_with(|| right.is_default.cmp(&left.is_default))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
                .then(Ordering::Equal)
        });
        exact_records.truncate(PERSONAL_MEMORY_QUERY_LIMIT);
        if !exact_records.is_empty() {
            return Ok(QueryPersonalMemoryResult::Matches(exact_records));
        }

        Ok(QueryPersonalMemoryResult::NoMatch)
    }

    pub(crate) async fn list(&self) -> Result<Vec<PersonalMemoryRecord>, PersonalMemoryError> {
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.persistence
            .send(ModelEvent::ListPersonalMemories(ListPersonalMemories {
                acknowledgement,
            }))
            .map_err(|_| PersonalMemoryError::PersistenceUnavailable)?;
        acknowledged
            .await
            .map_err(|_| PersonalMemoryError::PersistenceAcknowledgementDropped)?
            .map_err(Into::into)
    }
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub(crate) enum PersonalMemoryError {
    #[error("Personal Memory persistence is unavailable")]
    PersistenceUnavailable,
    #[error("Personal Memory persistence acknowledgement was dropped")]
    PersistenceAcknowledgementDropped,
    #[error("Personal Memory input is invalid")]
    InvalidInput,
    #[error("Personal Memory query is invalid")]
    InvalidQuery,
    #[error("Personal Memory has reached its 5,000-record capacity")]
    CapacityReached,
    #[error("Personal Memory persistence failed")]
    Persistence,
}

impl From<PersonalMemoryPersistenceError> for PersonalMemoryError {
    fn from(error: PersonalMemoryPersistenceError) -> Self {
        match error {
            PersonalMemoryPersistenceError::CapacityReached => Self::CapacityReached,
            PersonalMemoryPersistenceError::InvalidData => Self::InvalidInput,
            PersonalMemoryPersistenceError::Persistence => Self::Persistence,
        }
    }
}

pub(crate) fn normalize_search_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn validate_create_input(input: &CreatePersonalMemoryInput) -> Result<(), PersonalMemoryError> {
    let valid = !input.fact_text.trim().is_empty()
        && input.fact_text.chars().count() <= 4_096
        && !input.value_text.trim().is_empty()
        && input.value_text.chars().count() <= 2_048
        && !input.topic.trim().is_empty()
        && input.topic.chars().count() <= 512
        && input
            .labels
            .iter()
            .all(|label| !label.trim().is_empty() && label.chars().count() <= 128);
    valid.then_some(()).ok_or(PersonalMemoryError::InvalidInput)
}

fn match_score(record: &PersonalMemoryRecord, query: &str) -> Option<u8> {
    let topic = &record.normalized_topic;
    let value = normalize_search_text(&record.value_text);
    let fact = normalize_search_text(&record.fact_text);
    if topic == query || value == query {
        Some(0)
    } else if query.contains(topic) || topic.contains(query) {
        Some(1)
    } else if fact.contains(query) || query.contains(&fact) {
        Some(2)
    } else if value.contains(query) || query.contains(&value) {
        Some(3)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "personal_memory_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "capability_tests.rs"]
mod capability_tests;
