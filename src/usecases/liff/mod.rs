pub mod accept_terms;
pub mod create_payment;
pub mod end_session;
pub mod get_credit_balance;
pub mod get_credit_transactions;
pub mod get_current_session;
pub mod get_legal_doc;
pub mod get_me;
pub mod get_payment_status;
pub mod get_profile;
pub mod get_scenes;
pub mod get_tags;
pub mod require_active_user;
pub mod start_session;
pub mod track_events;

use crate::domain::repositories::RepoError;
use crate::usecases::UsecaseError;

pub(crate) const REPLY_IN_FLIGHT_MESSAGE: &str = "ตัวละครกำลังตอบอยู่ รอสักครู่แล้วลองใหม่";

/// A reply can start between the in-flight check and the story update; the DB trigger
/// `protect_pending_story` then raises `TURN_IN_PROGRESS`, which is the same user-facing case.
pub(crate) fn story_update_error(err: RepoError) -> UsecaseError {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(&err);
    while let Some(e) = source {
        if e.to_string().contains("TURN_IN_PROGRESS") {
            return UsecaseError::Validation(REPLY_IN_FLIGHT_MESSAGE.into());
        }
        source = e.source();
    }
    err.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::result::{DatabaseErrorKind, Error as DieselError};

    fn db_error(message: &str) -> RepoError {
        RepoError::Db {
            op: "roleplay_session.update",
            source: anyhow::Error::new(DieselError::DatabaseError(
                DatabaseErrorKind::Unknown,
                Box::new(message.to_string()),
            )),
        }
    }

    #[test]
    fn trigger_rejection_becomes_validation() {
        assert!(matches!(
            story_update_error(db_error("TURN_IN_PROGRESS")),
            UsecaseError::Validation(m) if m == REPLY_IN_FLIGHT_MESSAGE
        ));
    }

    #[test]
    fn other_db_errors_stay_infra() {
        assert!(matches!(
            story_update_error(db_error("deadlock detected")),
            UsecaseError::Infra(_)
        ));
    }
}
