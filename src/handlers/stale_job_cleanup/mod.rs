use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::usecases::background::StaleJobCleanupUseCase;

pub fn spawn(
    cleanup_usecase: Arc<StaleJobCleanupUseCase>,
    cancel: CancellationToken,
    cleanup_interval_secs: u64,
    stale_threshold_secs: i64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        tracing::info!(
            "Stale job cleanup started (interval={}s, threshold={}s)",
            cleanup_interval_secs,
            stale_threshold_secs
        );
        let mut interval = tokio::time::interval(Duration::from_secs(cleanup_interval_secs));

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Stale job cleanup shutting down");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(e) = cleanup_usecase.cleanup_stale_jobs(stale_threshold_secs).await {
                        tracing::error!(error = %e, "Stale job cleanup failed");
                    }
                }
            }
        }
    })
}
