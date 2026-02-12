use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("Database operation '{op}' failed")]
    Db {
        op: &'static str,
        #[source]
        source: anyhow::Error,
    },

    #[error("Database operation '{op}' failed for entity {entity_id}")]
    DbWithEntity {
        op: &'static str,
        entity_id: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Unique constraint violation: {0}")]
    UniqueViolation(String),

    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),
}
