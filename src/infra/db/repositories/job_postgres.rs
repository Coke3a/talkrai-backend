use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::entities::Job;
use crate::domain::repositories::{JobRepository, RepoError};
use crate::domain::value_objects::{JobId, JobMode, JobStatus, SessionId, UserId};
use crate::infra::db::postgres_connection::PgPool;
use crate::infra::db::schema::jobs;

use super::error_mapping::{map_diesel_error, map_pool_error};

#[derive(Queryable, Selectable)]
#[diesel(table_name = jobs)]
struct JobRow {
    id: Uuid,
    session_id: Option<Uuid>,
    user_id: Uuid,
    line_user_id: String,
    user_message: String,
    mode: String,
    status: String,
    attempts: i32,
    max_attempts: i32,
    locked_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    failed_reason: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl JobRow {
    fn into_entity(self) -> Job {
        Job::from_existing(
            JobId::from_uuid(self.id),
            JobMode::from_str(&self.mode).expect("invalid job_mode in DB"),
            self.session_id.map(SessionId::from_uuid),
            UserId::from_uuid(self.user_id),
            self.line_user_id,
            self.user_message,
            JobStatus::from_str(&self.status).expect("invalid job_status in DB"),
            self.attempts,
            self.max_attempts,
            self.locked_at,
            self.completed_at,
            self.failed_reason,
            self.created_at,
            self.updated_at,
        )
    }
}

#[derive(Insertable)]
#[diesel(table_name = jobs)]
struct NewJobRow<'a> {
    id: &'a Uuid,
    session_id: Option<&'a Uuid>,
    user_id: &'a Uuid,
    line_user_id: &'a str,
    user_message: &'a str,
    mode: &'a str,
    status: &'a str,
    attempts: i32,
    max_attempts: i32,
    locked_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    failed_reason: Option<&'a str>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl<'a> NewJobRow<'a> {
    fn from_entity(entity: &'a Job) -> Self {
        Self {
            id: entity.id().as_uuid(),
            session_id: entity.session_id().map(|s| s.as_uuid()),
            user_id: entity.user_id().as_uuid(),
            line_user_id: entity.line_user_id(),
            user_message: entity.user_message(),
            mode: entity.mode().as_str(),
            status: entity.status().as_str(),
            attempts: entity.attempts(),
            max_attempts: entity.max_attempts(),
            locked_at: entity.locked_at().copied(),
            completed_at: entity.completed_at().copied(),
            failed_reason: entity.failed_reason(),
            created_at: *entity.created_at(),
            updated_at: *entity.updated_at(),
        }
    }
}

pub struct JobPostgres {
    pool: Arc<PgPool>,
}

impl JobPostgres {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl JobRepository for JobPostgres {
    async fn create(&self, job: &Job) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let new_row = NewJobRow::from_entity(job);

        diesel::insert_into(jobs::table)
            .values(&new_row)
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("job.create", e))?;

        Ok(())
    }

    async fn find_by_id(&self, id: &JobId) -> Result<Option<Job>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = jobs::table
            .find(id.as_uuid())
            .first::<JobRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("job.find_by_id", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn lock_pending_job(&self, job_id: &JobId) -> Result<Option<Job>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = jobs::table
            .find(job_id.as_uuid())
            .filter(jobs::status.eq("pending"))
            .for_update()
            .skip_locked()
            .first::<JobRow>(&mut conn)
            .await
            .optional()
            .map_err(|e| map_diesel_error("job.lock_pending_job", e))?;

        Ok(result.map(|row| row.into_entity()))
    }

    async fn find_and_lock_pending_jobs(&self, limit: i64) -> Result<Vec<Job>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let results = jobs::table
            .filter(jobs::status.eq("pending"))
            .order(jobs::created_at.asc())
            .limit(limit)
            .for_update()
            .skip_locked()
            .load::<JobRow>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("job.find_and_lock_pending_jobs", e))?;

        Ok(results.into_iter().map(|row| row.into_entity()).collect())
    }

    async fn find_stale_processing_jobs(
        &self,
        threshold_seconds: i64,
    ) -> Result<Vec<Job>, RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let threshold = Utc::now() - chrono::Duration::seconds(threshold_seconds);

        let results = jobs::table
            .filter(jobs::status.eq("processing"))
            .filter(jobs::locked_at.lt(threshold))
            .load::<JobRow>(&mut conn)
            .await
            .map_err(|e| map_diesel_error("job.find_stale_processing_jobs", e))?;

        Ok(results.into_iter().map(|row| row.into_entity()).collect())
    }

    async fn update(&self, job: &Job) -> Result<(), RepoError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let rows_affected = diesel::update(jobs::table.find(job.id().as_uuid()))
            .set((
                jobs::status.eq(job.status().as_str()),
                jobs::attempts.eq(job.attempts()),
                jobs::locked_at.eq(job.locked_at()),
                jobs::completed_at.eq(job.completed_at()),
                jobs::failed_reason.eq(job.failed_reason()),
                jobs::updated_at.eq(job.updated_at()),
            ))
            .execute(&mut conn)
            .await
            .map_err(|e| map_diesel_error("job.update", e))?;

        if rows_affected == 0 {
            return Err(RepoError::NotFound(format!(
                "Job {} not found",
                job.id()
            )));
        }

        Ok(())
    }
}
