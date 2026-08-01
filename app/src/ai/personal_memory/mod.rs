use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::mpsc::SyncSender;

use futures::channel::oneshot;
use uuid::Uuid;

mod capability;
mod embedding;
mod types;

pub(crate) use capability::MemoryCapability;
#[cfg(test)]
pub(crate) use embedding::EmbeddingClient;
pub(crate) use embedding::{cosine_similarity, EmbeddingProvider, SharedEmbeddingClient};
#[cfg(any(test, feature = "personal_memory"))]
pub(crate) use embedding::{EmbeddingConnectionTestResult, EmbeddingProviderError};
pub(crate) use types::{
    CreatePersonalMemoryInput, CreatePersonalMemoryResult, NewPersonalMemoryRecord,
    PersonalMemoryIndexState, PersonalMemoryRecord, PersonalMemoryVector,
    QueryPersonalMemoryResult, PERSONAL_MEMORY_QUERY_LIMIT, PERSONAL_MEMORY_RECORD_LIMIT,
};

use crate::persistence::{
    CreatePersonalMemory, ListPersonalMemories, ListPersonalMemoryVectors, ModelEvent,
    PersonalMemoryPersistenceError, UpsertPersonalMemoryVector,
};

const SEMANTIC_MATCH_THRESHOLD: f32 = 0.55;

#[derive(Clone)]
pub(crate) struct PersonalMemoryService {
    persistence: SyncSender<ModelEvent>,
    embedding: Option<SharedEmbeddingClient>,
}

impl PersonalMemoryService {
    pub(crate) fn new(persistence: SyncSender<ModelEvent>) -> Self {
        Self {
            persistence,
            embedding: None,
        }
    }

    pub(crate) fn with_embedding(
        persistence: SyncSender<ModelEvent>,
        embedding: SharedEmbeddingClient,
    ) -> Self {
        Self {
            persistence,
            embedding: Some(embedding),
        }
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
            index_state: if self.embedding.is_some() {
                PersonalMemoryIndexState::Pending
            } else {
                PersonalMemoryIndexState::Unconfigured
            },
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
        if let Some(embedding) = &self.embedding {
            let record = result.record();
            if record.index_state != PersonalMemoryIndexState::Ready {
                if let Ok(vector) = embedding.embed(record.fact_text.clone()).await {
                    let _ = self
                        .upsert_vector(
                            record.record_id.clone(),
                            embedding.index_identity().to_string(),
                            vector,
                        )
                        .await;
                }
            }
        }
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

        let Some(embedding) = &self.embedding else {
            return Ok(QueryPersonalMemoryResult::NoMatch);
        };
        let Ok(query_vector) = embedding.embed(query.to_string()).await else {
            return Ok(QueryPersonalMemoryResult::NoMatch);
        };
        let vectors = self.list_vectors(embedding.index_identity()).await?;
        let records_by_id = records
            .into_iter()
            .map(|record| (record.record_id.clone(), record))
            .collect::<HashMap<_, _>>();
        let mut semantic_records = vectors
            .into_iter()
            .filter_map(|vector| {
                let similarity = cosine_similarity(&query_vector, &vector.values)?;
                (similarity >= SEMANTIC_MATCH_THRESHOLD).then(|| {
                    records_by_id
                        .get(&vector.record_id)
                        .cloned()
                        .map(|record| (similarity, record))
                })?
            })
            .collect::<Vec<_>>();
        semantic_records.sort_by(|(left_score, left), (right_score, right)| {
            right_score
                .total_cmp(left_score)
                .then_with(|| right.is_default.cmp(&left.is_default))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        let records = semantic_records
            .into_iter()
            .take(PERSONAL_MEMORY_QUERY_LIMIT)
            .map(|(_, record)| record)
            .collect::<Vec<_>>();
        if records.is_empty() {
            Ok(QueryPersonalMemoryResult::NoMatch)
        } else {
            Ok(QueryPersonalMemoryResult::Matches(records))
        }
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

    async fn upsert_vector(
        &self,
        record_id: String,
        index_identity: String,
        vector: Vec<f32>,
    ) -> Result<(), PersonalMemoryError> {
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.persistence
            .send(ModelEvent::UpsertPersonalMemoryVector(
                UpsertPersonalMemoryVector {
                    record_id,
                    index_identity,
                    vector,
                    acknowledgement,
                },
            ))
            .map_err(|_| PersonalMemoryError::PersistenceUnavailable)?;
        acknowledged
            .await
            .map_err(|_| PersonalMemoryError::PersistenceAcknowledgementDropped)?
            .map_err(Into::into)
    }

    async fn list_vectors(
        &self,
        index_identity: &str,
    ) -> Result<Vec<PersonalMemoryVector>, PersonalMemoryError> {
        let (acknowledgement, acknowledged) = oneshot::channel();
        self.persistence
            .send(ModelEvent::ListPersonalMemoryVectors(
                ListPersonalMemoryVectors {
                    index_identity: index_identity.to_string(),
                    acknowledgement,
                },
            ))
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
