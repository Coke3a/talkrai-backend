use async_trait::async_trait;

use crate::domain::entities::Job;
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::JobId;

#[async_trait]
pub trait JobRepository: Send + Sync {
    async fn create(&self, job: &Job) -> Result<(), RepoError>;
    async fn find_by_id(&self, id: &JobId) -> Result<Option<Job>, RepoError>;

    /// Lock a pending job using SELECT FOR UPDATE SKIP LOCKED
    async fn lock_pending_job(&self, job_id: &JobId) -> Result<Option<Job>, RepoError>;

    /// Find and lock up to `limit` pending jobs
    async fn find_and_lock_pending_jobs(&self, limit: i64) -> Result<Vec<Job>, RepoError>;

    /// Find stale processing jobs (locked_at older than threshold)
    async fn find_stale_processing_jobs(
        &self,
        threshold_seconds: i64,
    ) -> Result<Vec<Job>, RepoError>;

    async fn update(&self, job: &Job) -> Result<(), RepoError>;
}
