use diesel::result::{DatabaseErrorKind, Error as DieselError};

use crate::domain::repositories::RepoError;

pub fn map_diesel_error(op: &'static str, err: DieselError) -> RepoError {
    match &err {
        DieselError::NotFound => RepoError::NotFound(format!("{} returned no rows", op)),
        DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, info) => {
            RepoError::UniqueViolation(info.message().to_string())
        }
        DieselError::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, info) => {
            RepoError::ForeignKeyViolation(info.message().to_string())
        }
        _ => RepoError::Db {
            op,
            source: anyhow::Error::new(err),
        },
    }
}

pub fn map_pool_error(err: impl std::error::Error + Send + Sync + 'static) -> RepoError {
    RepoError::ConnectionError(err.to_string())
}
