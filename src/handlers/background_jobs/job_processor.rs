use std::sync::Arc;

use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::domain::value_objects::JobId;
use crate::handlers::background_jobs::job_dispatcher::JobDispatcher;

pub fn spawn(
    dispatcher: Arc<JobDispatcher>,
    mut rx: mpsc::Receiver<JobId>,
    cancel: CancellationToken,
    max_concurrent: usize,
) -> JoinHandle<()> {
    let semaphore = Arc::new(Semaphore::new(max_concurrent));

    tokio::spawn(async move {
        tracing::info!("Job processor started (max_concurrent={})", max_concurrent);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("Job processor shutting down");
                    break;
                }
                result = rx.recv() => {
                    match result {
                        Some(job_id) => {
                            let permit = semaphore.clone().acquire_owned().await.unwrap();
                            let dispatcher = Arc::clone(&dispatcher);

                            tokio::spawn(async move {
                                let _permit = permit; // held until task completes
                                if let Err(e) = dispatcher
                                    .dispatch(job_id.clone())
                                    .await
                                {
                                    tracing::error!(
                                        job_id = %job_id.as_uuid(),
                                        error = %e,
                                        "Failed to process job"
                                    );
                                }
                            });
                        }
                        None => {
                            tracing::info!("Job processor channel closed");
                            break;
                        }
                    }
                }
            }
        }
    })
}
