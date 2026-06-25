use async_trait::async_trait;

use crate::domain::entities::Job;
use crate::domain::repositories::RepoError;
use crate::domain::value_objects::{JobId, SessionId};

#[async_trait]
pub trait JobRepository: Send + Sync {
    async fn create(&self, job: &Job) -> Result<(), RepoError>;

    /// Bulk-insert jobs in a single statement (used by the re-engagement enqueue).
    async fn create_many(&self, jobs: &[Job]) -> Result<(), RepoError>;

    async fn find_by_id(&self, id: &JobId) -> Result<Option<Job>, RepoError>;

    /// Read a still-pending job. Best-effort `SELECT … FOR UPDATE SKIP LOCKED` to reduce
    /// contention, but the durable claim is [`Self::mark_processing_if_pending`] — the row lock
    /// here does not survive past this call (each query commits in autocommit).
    async fn lock_pending_job(&self, job_id: &JobId) -> Result<Option<Job>, RepoError>;

    /// Atomically persist the pending→processing transition produced by [`Job::lock`], guarded on
    /// the DB row still being `pending` (`UPDATE … WHERE status = 'pending'`). Returns `true` if
    /// this worker won the claim, `false` if another worker claimed it first. This compare-and-set
    /// is what closes the lock-then-update race between the mpsc fast path and the DB poller.
    async fn mark_processing_if_pending(&self, job: &Job) -> Result<bool, RepoError>;

    /// Find and lock up to `limit` pending jobs
    async fn find_and_lock_pending_jobs(&self, limit: i64) -> Result<Vec<Job>, RepoError>;

    /// Find stale processing jobs (locked_at older than threshold)
    async fn find_stale_processing_jobs(
        &self,
        threshold_seconds: i64,
    ) -> Result<Vec<Job>, RepoError>;

    /// Check if the given session already has a non-terminal (pending or processing) job.
    async fn has_active_job_for_session(&self, session_id: &SessionId) -> Result<bool, RepoError>;

    async fn update(&self, job: &Job) -> Result<(), RepoError>;
}
