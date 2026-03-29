use std::sync::Arc;

use crate::domain::repositories::JobRepository;
use crate::domain::value_objects::{JobId, JobMode};
use crate::usecases::webhook::process_roleplay_message::{
    ProcessRoleplayMessageInput, ProcessRoleplayMessageUseCase,
};
use crate::usecases::UsecaseError;

pub struct JobDispatcher {
    job_repo: Arc<dyn JobRepository>,
    roleplay_usecase: Arc<ProcessRoleplayMessageUseCase>,
}

impl JobDispatcher {
    pub fn new(
        job_repo: Arc<dyn JobRepository>,
        roleplay_usecase: Arc<ProcessRoleplayMessageUseCase>,
    ) -> Self {
        Self {
            job_repo,
            roleplay_usecase,
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
        self.job_repo.update(&job).await?;

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
