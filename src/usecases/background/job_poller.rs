use std::sync::Arc;

use crate::domain::repositories::JobRepository;
use crate::domain::value_objects::JobId;
use crate::usecases::UsecaseError;

pub struct JobPollerUseCase {
    job_repo: Arc<dyn JobRepository>,
}

impl JobPollerUseCase {
    pub fn new(job_repo: Arc<dyn JobRepository>) -> Self {
        Self { job_repo }
    }

    pub async fn poll_pending_jobs(&self, limit: i64) -> Result<Vec<JobId>, UsecaseError> {
        let jobs = self.job_repo.find_and_lock_pending_jobs(limit).await?;

        let job_ids: Vec<JobId> = jobs.iter().map(|j| j.id().clone()).collect();

        if !job_ids.is_empty() {
            tracing::info!(count = job_ids.len(), "Polled pending jobs from DB");
        }

        Ok(job_ids)
    }
}
