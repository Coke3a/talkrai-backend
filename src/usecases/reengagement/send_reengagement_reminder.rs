use std::sync::Arc;

use crate::domain::entities::Job;
use crate::domain::repositories::{CharacterRepository, JobRepository, RoleplaySessionRepository};
use crate::domain::services::line_client::{LineClient, LineMessage};
use crate::domain::value_objects::JobId;
use crate::infra::line::retention_flex;
use crate::usecases::UsecaseError;

pub struct SendReengagementReminderInput {
    pub job_id: JobId,
}

/// Per-job worker for a `ReengagementReminder` job (spec §C.5): if the user has an active
/// (resumable) session, builds the reminder flex for that character and pushes it via LINE.
pub struct SendReengagementReminderUseCase {
    job_repo: Arc<dyn JobRepository>,
    session_repo: Arc<dyn RoleplaySessionRepository>,
    character_repo: Arc<dyn CharacterRepository>,
    line_client: Arc<dyn LineClient>,
}

impl SendReengagementReminderUseCase {
    pub fn new(
        job_repo: Arc<dyn JobRepository>,
        session_repo: Arc<dyn RoleplaySessionRepository>,
        character_repo: Arc<dyn CharacterRepository>,
        line_client: Arc<dyn LineClient>,
    ) -> Self {
        Self {
            job_repo,
            session_repo,
            character_repo,
            line_client,
        }
    }

    pub async fn execute(&self, input: SendReengagementReminderInput) -> Result<(), UsecaseError> {
        let mut job = self
            .job_repo
            .lock_pending_job(&input.job_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Job not found or already locked".into()))?;
        job.lock()?;
        // Atomic claim: bail if another worker already claimed this job between our read and now,
        // so a re-engagement reminder is never pushed twice.
        if !self.job_repo.mark_processing_if_pending(&job).await? {
            return Err(UsecaseError::NotFound(
                "Job already claimed by another worker".into(),
            ));
        }

        match self.process(&job).await {
            Ok(()) => {
                job.complete()?;
                self.job_repo.update(&job).await?;
                Ok(())
            }
            Err(e) => {
                tracing::warn!(
                    job_id = %job.id().as_uuid(),
                    user_id = %job.user_id().as_uuid(),
                    error = %e,
                    "Re-engagement reminder failed"
                );
                let _ = job.fail(e.to_string());
                let _ = self.job_repo.update(&job).await;
                Err(e)
            }
        }
    }

    async fn process(&self, job: &Job) -> Result<(), UsecaseError> {
        // Only remind users with an ACTIVE (resumable) session: the "story still waiting" copy and
        // the "กลับมาแล้ว" CTA only make sense mid-story. The webhook resumes an active session, but
        // shows a scene-picker when there is none — so skip here otherwise (an acceptable miss per
        // spec §C.5; the enqueue already marked the user reminded today).
        let session = match self
            .session_repo
            .find_active_by_user_id(job.user_id())
            .await?
        {
            Some(s) => s,
            None => {
                tracing::info!(
                    user_id = %job.user_id().as_uuid(),
                    "No active session to resume; skipping reminder"
                );
                return Ok(());
            }
        };

        let character = match self
            .character_repo
            .find_by_id(session.character_id())
            .await?
        {
            Some(c) => c,
            None => {
                tracing::info!(
                    user_id = %job.user_id().as_uuid(),
                    "Character missing; skipping reminder"
                );
                return Ok(());
            }
        };

        let flex = retention_flex::build_reengagement_flex(
            character.name().as_str(),
            character.avatar_url(),
        );
        let alt_text = format!("{}ยังรอคำตอบของคุณอยู่...", character.name().as_str());

        self.line_client
            .push_messages(
                job.line_user_id(),
                vec![LineMessage::Flex {
                    alt_text,
                    contents: flex,
                    sender_name: character.name().as_str().to_string(),
                    sender_icon_url: character.avatar_url().unwrap_or_default().to_string(),
                    quick_reply: None,
                }],
            )
            .await
            .map_err(UsecaseError::from)?;

        tracing::info!(
            job_id = %job.id().as_uuid(),
            user_id = %job.user_id().as_uuid(),
            "Re-engagement reminder pushed"
        );
        Ok(())
    }
}
