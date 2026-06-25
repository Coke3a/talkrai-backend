use std::sync::Arc;

use chrono::NaiveDate;

use crate::domain::entities::Job;
use crate::domain::repositories::{AppConfigRepository, JobRepository, UserRepository};
use crate::domain::value_objects::{JobMode, UserId};
use crate::usecases::UsecaseError;

const DEFAULT_WINDOW_DAYS: i64 = 14;
const DEFAULT_BATCH_CAP: i64 = 2000;

pub struct EnqueueReengagementResult {
    pub enqueued: usize,
    pub capped: bool,
}

/// Selects re-engagement targets and enqueues one reminder job per target (spec §C.5).
/// Only enqueues — the actual LINE pushes run through the existing job pipeline.
pub struct EnqueueReengagementRemindersUseCase {
    user_repo: Arc<dyn UserRepository>,
    job_repo: Arc<dyn JobRepository>,
    config_repo: Arc<dyn AppConfigRepository>,
}

impl EnqueueReengagementRemindersUseCase {
    pub fn new(
        user_repo: Arc<dyn UserRepository>,
        job_repo: Arc<dyn JobRepository>,
        config_repo: Arc<dyn AppConfigRepository>,
    ) -> Self {
        Self {
            user_repo,
            job_repo,
            config_repo,
        }
    }

    /// `today` is the Asia/Bangkok date (supplied by the caller via `bangkok_today()`).
    pub async fn execute(
        &self,
        today: NaiveDate,
    ) -> Result<EnqueueReengagementResult, UsecaseError> {
        let window_days = self
            .config_i64("reengagement_window_days", DEFAULT_WINDOW_DAYS)
            .await?;
        let batch_cap = self
            .config_i64("reengagement_batch_cap", DEFAULT_BATCH_CAP)
            .await?;

        let targets = self
            .user_repo
            .find_reengagement_targets(today, window_days, batch_cap)
            .await?;

        if targets.is_empty() {
            return Ok(EnqueueReengagementResult {
                enqueued: 0,
                capped: false,
            });
        }

        // Mark reminded first (idempotency anchor): a crash before enqueue under-notifies — a missed
        // evening nudge, acceptable per spec — rather than risking a double-notify on the next run.
        let user_ids: Vec<UserId> = targets.iter().map(|t| t.user_id.clone()).collect();
        self.user_repo.mark_reminded(&user_ids, today).await?;

        let jobs: Vec<Job> = targets
            .iter()
            .map(|t| {
                Job::new(
                    JobMode::ReengagementReminder,
                    None,
                    t.user_id.clone(),
                    t.line_user_id.clone(),
                    String::new(),
                )
            })
            .collect();
        let enqueued = jobs.len();
        self.job_repo.create_many(&jobs).await?;

        let capped = enqueued as i64 >= batch_cap;
        if capped {
            tracing::warn!(
                enqueued,
                batch_cap,
                "re-engagement batch hit the cap; remainder deferred to the next run"
            );
        }

        Ok(EnqueueReengagementResult { enqueued, capped })
    }

    async fn config_i64(&self, key: &str, default: i64) -> Result<i64, UsecaseError> {
        let raw = self.config_repo.get(key).await?;
        Ok(raw.and_then(|v| v.parse().ok()).unwrap_or(default))
    }
}
