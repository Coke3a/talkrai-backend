use std::sync::Arc;

use crate::domain::repositories::JobRepository;
use crate::domain::value_objects::{JobId, JobMode};
use crate::usecases::reengagement::send_reengagement_reminder::{
    SendReengagementReminderInput, SendReengagementReminderUseCase,
};
use crate::usecases::webhook::process_roleplay_message::{
    ProcessRoleplayMessageInput, ProcessRoleplayMessageUseCase,
};
use crate::usecases::UsecaseError;

pub struct JobDispatcher {
    job_repo: Arc<dyn JobRepository>,
    roleplay_usecase: Arc<ProcessRoleplayMessageUseCase>,
    reengagement_usecase: Arc<SendReengagementReminderUseCase>,
}

impl JobDispatcher {
    pub fn new(
        job_repo: Arc<dyn JobRepository>,
        roleplay_usecase: Arc<ProcessRoleplayMessageUseCase>,
        reengagement_usecase: Arc<SendReengagementReminderUseCase>,
    ) -> Self {
        Self {
            job_repo,
            roleplay_usecase,
            reengagement_usecase,
        }
    }

    pub async fn dispatch(&self, job_id: JobId) -> Result<(), UsecaseError> {
        let job = self
            .job_repo
            .find_by_id(&job_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Job not found".into()))?;

        match job.mode() {
            JobMode::RoleplayMessage => {
                self.roleplay_usecase
                    .execute(ProcessRoleplayMessageInput { job_id })
                    .await
            }
            JobMode::ReengagementReminder => {
                self.reengagement_usecase
                    .execute(SendReengagementReminderInput { job_id })
                    .await
            }
            other => self.handle_stub(&job_id, other).await,
        }
    }

    async fn handle_stub(&self, job_id: &JobId, mode: &JobMode) -> Result<(), UsecaseError> {
        let mut job = self
            .job_repo
            .lock_pending_job(job_id)
            .await?
            .ok_or_else(|| UsecaseError::NotFound("Job not found or already locked".into()))?;

        job.lock()?;
        // Atomic claim — bail if another worker already took this job (see JobRepository docs).
        if !self.job_repo.mark_processing_if_pending(&job).await? {
            return Err(UsecaseError::NotFound(
                "Job already claimed by another worker".into(),
            ));
        }

        tracing::info!(
            job_id = %job_id.as_uuid(),
            mode = mode.as_str(),
            "Stub handler — auto-completing job"
        );

        job.complete()?;
        self.job_repo.update(&job).await?;
        Ok(())
    }
}
