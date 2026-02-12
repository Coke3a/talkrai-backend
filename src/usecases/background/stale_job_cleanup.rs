use std::sync::Arc;

use crate::domain::repositories::JobRepository;
use crate::usecases::UsecaseError;

pub struct StaleJobCleanupUseCase {
    job_repo: Arc<dyn JobRepository>,
}

impl StaleJobCleanupUseCase {
    pub fn new(job_repo: Arc<dyn JobRepository>) -> Self {
        Self { job_repo }
    }

    pub async fn cleanup_stale_jobs(&self, threshold_seconds: i64) -> Result<u64, UsecaseError> {
        let stale_jobs = self
            .job_repo
            .find_stale_processing_jobs(threshold_seconds)
            .await?;

        let mut cleaned = 0u64;

        for mut job in stale_jobs {
            if job.can_retry() {
                job.reset_to_pending()?;
                tracing::warn!(
                    job_id = %job.id().as_uuid(),
                    attempts = job.attempts(),
                    "Reset stale job to pending"
                );
            } else {
                job.fail("Exceeded max attempts after stale lock".to_string())?;
                tracing::error!(
                    job_id = %job.id().as_uuid(),
                    "Failed stale job — exceeded max attempts"
                );
            }
            self.job_repo.update(&job).await?;
            cleaned += 1;
        }

        if cleaned > 0 {
            tracing::info!(count = cleaned, "Cleaned up stale jobs");
        }

        Ok(cleaned)
    }
}
