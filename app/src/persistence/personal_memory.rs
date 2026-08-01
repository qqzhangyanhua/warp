use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::SqliteConnection;
use futures::channel::oneshot;

use super::schema::{personal_memory_records, personal_memory_vectors};
use crate::ai::personal_memory::{
    CreatePersonalMemoryResult, NewPersonalMemoryRecord, PersonalMemoryIndexState,
    PersonalMemoryRecord, PersonalMemoryVector, PERSONAL_MEMORY_RECORD_LIMIT,
};

#[derive(Debug)]
pub struct CreatePersonalMemory {
    pub(crate) record: NewPersonalMemoryRecord,
    pub(crate) acknowledgement:
        oneshot::Sender<Result<CreatePersonalMemoryResult, PersonalMemoryPersistenceError>>,
}

#[derive(Debug)]
pub struct ListPersonalMemories {
    pub(crate) acknowledgement:
        oneshot::Sender<Result<Vec<PersonalMemoryRecord>, PersonalMemoryPersistenceError>>,
}

#[derive(Debug)]
pub struct UpsertPersonalMemoryVector {
    pub(crate) record_id: String,
    pub(crate) index_identity: String,
    pub(crate) vector: Vec<f32>,
    pub(crate) acknowledgement: oneshot::Sender<Result<(), PersonalMemoryPersistenceError>>,
}

#[derive(Debug)]
pub struct ListPersonalMemoryVectors {
    pub(crate) index_identity: String,
    pub(crate) acknowledgement:
        oneshot::Sender<Result<Vec<PersonalMemoryVector>, PersonalMemoryPersistenceError>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PersonalMemoryPersistenceError {
    #[error("Personal Memory has reached its record capacity")]
    CapacityReached,
    #[error("Personal Memory data is invalid")]
    InvalidData,
    #[error("Personal Memory persistence failed")]
    Persistence,
}

impl From<diesel::result::Error> for PersonalMemoryPersistenceError {
    fn from(_: diesel::result::Error) -> Self {
        Self::Persistence
    }
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = personal_memory_records)]
struct PersonalMemoryRow {
    id: i32,
    record_id: String,
    fact_text: String,
    value_text: String,
    topic: String,
    normalized_topic: String,
    labels_json: String,
    is_default: bool,
    index_state: String,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = personal_memory_records)]
struct NewPersonalMemoryRow<'a> {
    record_id: &'a str,
    fact_text: &'a str,
    value_text: &'a str,
    topic: &'a str,
    normalized_topic: &'a str,
    labels_json: &'a str,
    is_default: bool,
    index_state: &'a str,
}

#[derive(Insertable)]
#[diesel(table_name = personal_memory_vectors)]
struct NewPersonalMemoryVectorRow<'a> {
    record_id: &'a str,
    index_identity: &'a str,
    dimensions: i32,
    vector: &'a [u8],
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = personal_memory_vectors)]
struct PersonalMemoryVectorRow {
    record_id: String,
    dimensions: i32,
    vector: Vec<u8>,
}

pub(super) fn create_personal_memory(
    conn: &mut SqliteConnection,
    command: &CreatePersonalMemory,
) -> Result<CreatePersonalMemoryResult, PersonalMemoryPersistenceError> {
    conn.transaction::<_, PersonalMemoryPersistenceError, _>(|conn| {
        use personal_memory_records::dsl;

        let existing = dsl::personal_memory_records
            .filter(dsl::fact_text.eq(&command.record.fact_text))
            .filter(dsl::value_text.eq(&command.record.value_text))
            .filter(dsl::normalized_topic.eq(&command.record.normalized_topic))
            .select(PersonalMemoryRow::as_select())
            .first::<PersonalMemoryRow>(conn)
            .optional()
            .map_err(|_| PersonalMemoryPersistenceError::Persistence)?;
        if let Some(existing) = existing {
            return Ok(CreatePersonalMemoryResult::AlreadyRemembered(
                existing.try_into()?,
            ));
        }

        let count = dsl::personal_memory_records
            .count()
            .get_result::<i64>(conn)
            .map_err(|_| PersonalMemoryPersistenceError::Persistence)?;
        if count >= PERSONAL_MEMORY_RECORD_LIMIT {
            return Err(PersonalMemoryPersistenceError::CapacityReached);
        }

        let labels_json = serde_json::to_string(&command.record.labels)
            .map_err(|_| PersonalMemoryPersistenceError::InvalidData)?;
        let row = NewPersonalMemoryRow {
            record_id: &command.record.record_id,
            fact_text: &command.record.fact_text,
            value_text: &command.record.value_text,
            topic: &command.record.topic,
            normalized_topic: &command.record.normalized_topic,
            labels_json: &labels_json,
            is_default: command.record.is_default,
            index_state: command.record.index_state.as_database_value(),
        };
        diesel::insert_into(dsl::personal_memory_records)
            .values(row)
            .execute(conn)
            .map_err(|_| PersonalMemoryPersistenceError::Persistence)?;
        let inserted = dsl::personal_memory_records
            .filter(dsl::record_id.eq(&command.record.record_id))
            .select(PersonalMemoryRow::as_select())
            .first::<PersonalMemoryRow>(conn)
            .map_err(|_| PersonalMemoryPersistenceError::Persistence)?;
        Ok(CreatePersonalMemoryResult::Created(inserted.try_into()?))
    })
}

pub(super) fn list_personal_memories(
    conn: &mut SqliteConnection,
) -> Result<Vec<PersonalMemoryRecord>, PersonalMemoryPersistenceError> {
    use personal_memory_records::dsl;

    dsl::personal_memory_records
        .order((dsl::updated_at.desc(), dsl::id.desc()))
        .select(PersonalMemoryRow::as_select())
        .load::<PersonalMemoryRow>(conn)
        .map_err(|_| PersonalMemoryPersistenceError::Persistence)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
}

pub(super) fn upsert_personal_memory_vector(
    conn: &mut SqliteConnection,
    command: &UpsertPersonalMemoryVector,
) -> Result<(), PersonalMemoryPersistenceError> {
    if command.index_identity.len() != 64
        || !command
            .index_identity
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || command.vector.is_empty()
        || command.vector.len() > 8_192
        || command.vector.iter().any(|value| !value.is_finite())
    {
        return Err(PersonalMemoryPersistenceError::InvalidData);
    }
    let dimensions = i32::try_from(command.vector.len())
        .map_err(|_| PersonalMemoryPersistenceError::InvalidData)?;
    let bytes = command
        .vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    conn.transaction::<_, PersonalMemoryPersistenceError, _>(|conn| {
        use personal_memory_records::dsl as records;
        use personal_memory_vectors::dsl as vectors;

        diesel::insert_into(vectors::personal_memory_vectors)
            .values(NewPersonalMemoryVectorRow {
                record_id: &command.record_id,
                index_identity: &command.index_identity,
                dimensions,
                vector: &bytes,
            })
            .on_conflict(vectors::record_id)
            .do_update()
            .set((
                vectors::index_identity.eq(&command.index_identity),
                vectors::dimensions.eq(dimensions),
                vectors::vector.eq(&bytes),
                vectors::updated_at.eq(diesel::dsl::now),
            ))
            .execute(conn)?;
        let updated = diesel::update(
            records::personal_memory_records.filter(records::record_id.eq(&command.record_id)),
        )
        .set(records::index_state.eq(PersonalMemoryIndexState::Ready.as_database_value()))
        .execute(conn)?;
        if updated != 1 {
            return Err(PersonalMemoryPersistenceError::InvalidData);
        }
        Ok(())
    })
}

pub(super) fn list_personal_memory_vectors(
    conn: &mut SqliteConnection,
    command: &ListPersonalMemoryVectors,
) -> Result<Vec<PersonalMemoryVector>, PersonalMemoryPersistenceError> {
    use personal_memory_vectors::dsl;

    dsl::personal_memory_vectors
        .filter(dsl::index_identity.eq(&command.index_identity))
        .select(PersonalMemoryVectorRow::as_select())
        .load::<PersonalMemoryVectorRow>(conn)?
        .into_iter()
        .map(|row| {
            let expected_bytes = usize::try_from(row.dimensions)
                .ok()
                .and_then(|dimensions| dimensions.checked_mul(std::mem::size_of::<f32>()))
                .ok_or(PersonalMemoryPersistenceError::InvalidData)?;
            if row.vector.len() != expected_bytes {
                return Err(PersonalMemoryPersistenceError::InvalidData);
            }
            let values = row
                .vector
                .chunks_exact(4)
                .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
                .collect::<Vec<_>>();
            if values.iter().any(|value| !value.is_finite()) {
                return Err(PersonalMemoryPersistenceError::InvalidData);
            }
            Ok(PersonalMemoryVector {
                record_id: row.record_id,
                values,
            })
        })
        .collect()
}

impl TryFrom<PersonalMemoryRow> for PersonalMemoryRecord {
    type Error = PersonalMemoryPersistenceError;

    fn try_from(row: PersonalMemoryRow) -> Result<Self, Self::Error> {
        let labels = serde_json::from_str(&row.labels_json)
            .map_err(|_| PersonalMemoryPersistenceError::InvalidData)?;
        let index_state = PersonalMemoryIndexState::from_database_value(&row.index_state)
            .ok_or(PersonalMemoryPersistenceError::InvalidData)?;
        let _ = row.id;
        Ok(Self {
            record_id: row.record_id,
            fact_text: row.fact_text,
            value_text: row.value_text,
            topic: row.topic,
            normalized_topic: row.normalized_topic,
            labels,
            is_default: row.is_default,
            index_state,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[cfg(test)]
#[path = "personal_memory_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "personal_memory_migration_tests.rs"]
mod migration_tests;
