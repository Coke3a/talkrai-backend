use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::domain::value_objects::JobId;
use crate::usecases::background_jobs::JobPollerUseCase;

pub fn spawn(
    poller_usecase: Arc<JobPollerUseCase>,
    tx: mpsc::Sender<JobId>,
    cancel: CancellationToken,
    poll_interval_secs: u64,
    poll_batch_size: i64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(
            "Job poller started (interval={}s, batch_size={})",
            poll_interval_secs,
            poll_batch_size
        );
        let mut interval = tokio::time::interval(Duration::from_secs(poll_interval_secs));

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Job poller shutting down");
                    break;
                }
                _ = interval.tick() => {
                    match poller_usecase.poll_pending_jobs(poll_batch_size).await {
                        Ok(job_ids) => {
                            for job_id in job_ids {
                                if let Err(e) = tx.try_send(job_id) {
                                    let skipped_job_id = e.into_inner();
                                    tracing::warn!(
                                        job_id = %skipped_job_id.as_uuid(),
                                        "Job poller: channel full, skipping job"
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Job poller failed");
                        }
                    }
                }
            }
        }
    })
}
