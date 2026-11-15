//! Two-phase browser preview: `POST/GET/content/confirm/DELETE` for
//! `/api/v1/previews`. The background preview worker in this module calls
//! the exact same [`crate::processing::process_input`] the direct-batch
//! worker calls (B1) — this file owns preview-specific orchestration
//! (routing, worker loop, TTL sweep, confirm response shaping) but never a
//! second copy of parsing/OCR/masking.

use std::convert::Infallible;
use std::time::Duration;

use engine_core::InputFormat;
use warp::{http::StatusCode, Filter, Rejection, Reply};

use crate::{store::ConfirmOutcome, Runtime};

pub fn routes<F>(
    runtime_filter: F,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone
where
    F: Filter<Extract = (Runtime,), Error = Infallible> + Clone + Send + Sync + 'static,
{
    let create = warp::path!("api" / "v1" / "previews")
        .and(warp::post())
        .and(warp::multipart::form().max_length(2 * 1024 * 1024 * 1024 + 1024 * 1024))
        .and(runtime_filter.clone())
        .and_then(create_preview_handler);

    let detail = warp::path!("api" / "v1" / "previews" / String)
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(detail_handler);

    let content = warp::path!("api" / "v1" / "previews" / String / "files" / String / "content")
        .and(warp::get())
        .and(runtime_filter.clone())
        .and_then(content_handler);

    let confirm = warp::path!("api" / "v1" / "previews" / String / "confirm")
        .and(warp::post())
        .and(runtime_filter.clone())
        .and_then(confirm_handler);

    let delete = warp::path!("api" / "v1" / "previews" / String)
        .and(warp::delete())
        .and(runtime_filter)
        .and_then(delete_handler);

    create.or(detail).or(content).or(confirm).or(delete)
}

async fn create_preview_handler(
    form: warp::multipart::FormData,
    runtime: Runtime,
) -> Result<impl Reply, Rejection> {
    let _ = runtime.store.sweep_expired_previews().await;
    let (uploads, rules, _restore_mode) = crate::parse_form(form, &runtime).await?;
    let response = runtime
        .store
        .create_preview(uploads, rules, runtime.preview_ttl)
        .await
        .map_err(crate::store_rejection)?;
    runtime.wake_preview_worker.notify_one();
    Ok(warp::reply::with_status(
        warp::reply::json(&response),
        StatusCode::ACCEPTED,
    ))
}

async fn detail_handler(preview_id: String, runtime: Runtime) -> Result<impl Reply, Rejection> {
    let _ = runtime.store.sweep_expired_previews().await;
    let detail = runtime
        .store
        .preview_detail(&preview_id)
        .await
        .map_err(crate::store_rejection)?;
    Ok(warp::reply::json(&detail))
}

async fn content_handler(
    preview_id: String,
    file_id: String,
    runtime: Runtime,
) -> Result<impl Reply, Rejection> {
    let _ = runtime.store.sweep_expired_previews().await;
    let content = runtime
        .store
        .preview_file_content(&preview_id, &file_id)
        .await
        .map_err(crate::store_rejection)?;
    Ok(warp::http::Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/markdown; charset=utf-8")
        .header("cache-control", "no-store")
        .body(content)
        .expect("valid preview content response"))
}

async fn confirm_handler(preview_id: String, runtime: Runtime) -> Result<impl Reply, Rejection> {
    let _ = runtime.store.sweep_expired_previews().await;
    match runtime.store.confirm_preview(&preview_id).await {
        Ok(ConfirmOutcome::Confirmed { batch_id })
        | Ok(ConfirmOutcome::AlreadyConfirmed { batch_id }) => {
            // The new batch may contain Failed files; wake the batch worker
            // in case a future retry is queued against it.
            runtime.wake_worker.notify_one();
            Ok(warp::reply::json(
                &service_contracts::ConfirmPreviewResponse {
                    preview_id,
                    batch_id,
                },
            ))
        }
        Ok(ConfirmOutcome::InFlight) => Err(crate::api_error(
            StatusCode::CONFLICT,
            "PREVIEW_CONFIRM_IN_PROGRESS",
            "A confirm is already in progress for this preview",
            true,
        )),
        Ok(ConfirmOutcome::NotReady) => Err(crate::api_error(
            StatusCode::CONFLICT,
            "PREVIEW_NOT_READY",
            "Preview is not ready to confirm",
            false,
        )),
        Err(error) => Err(crate::store_rejection(error)),
    }
}

async fn delete_handler(preview_id: String, runtime: Runtime) -> Result<impl Reply, Rejection> {
    runtime
        .store
        .delete_preview(&preview_id)
        .await
        .map_err(crate::store_rejection)?;
    Ok(warp::reply::with_status(
        warp::reply(),
        StatusCode::NO_CONTENT,
    ))
}

impl Runtime {
    pub(crate) fn spawn_preview_worker(&self) {
        let runtime = self.clone();
        tokio::spawn(async move {
            loop {
                match runtime.store.claim_next_pending_preview_file().await {
                    Ok(Some(job)) => runtime.process_preview_job(job).await,
                    Ok(None) => {
                        tokio::select! {
                            _ = runtime.wake_preview_worker.notified() => {},
                            _ = tokio::time::sleep(Duration::from_millis(200)) => {},
                        }
                    }
                    Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
                }
            }
        });
    }

    /// Periodic background TTL sweep (D2). The interval scales down with a
    /// short test-injected TTL so short-TTL tests can observe expiry quickly,
    /// while production (30-minute TTL) sweeps at a steady 60-second cadence.
    pub(crate) fn spawn_preview_sweeper(&self, ttl: Duration) {
        let runtime = self.clone();
        let interval = (ttl / 4)
            .min(Duration::from_secs(60))
            .max(Duration::from_millis(50));
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let _ = runtime.store.sweep_expired_previews().await;
            }
        });
    }

    async fn process_preview_job(&self, job: crate::store::PendingPreviewJob) {
        let bytes = match self.store.read_input(&job.input_object_key).await {
            Ok(bytes) => bytes,
            Err(_) => {
                let _ = self
                    .store
                    .mark_preview_file_failed(
                        &job.preview_id,
                        &job.file_id,
                        "INPUT_READ_FAILED",
                        "Input could not be read",
                    )
                    .await;
                return;
            }
        };
        let input_format = match InputFormat::parse(&job.input_format) {
            Some(format) => format,
            None => {
                let _ = self
                    .store
                    .mark_preview_file_failed(
                        &job.preview_id,
                        &job.file_id,
                        "INPUT_FORMAT_UNSUPPORTED",
                        "The stored input format is not supported",
                    )
                    .await;
                return;
            }
        };

        self.processing_call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let result = match crate::processing::process_input(crate::processing::ProcessingInput {
            bytes: &bytes,
            display_name: &job.display_name,
            input_format,
            rule_ids: &job.rules,
            ocr_config: self.ocr_config.as_ref(),
        })
        .await
        {
            Ok(result) => result,
            Err(failure) => {
                let _ = self
                    .store
                    .mark_preview_file_failed(
                        &job.preview_id,
                        &job.file_id,
                        &failure.code,
                        &failure.message,
                    )
                    .await;
                return;
            }
        };

        let artifact_id = uuid::Uuid::new_v4().to_string();
        let mapping_bytes = {
            use engine_core::{
                encode_server_cmap, ServerCmap, SERVER_CMAP_MAGIC, SERVER_CMAP_VERSION,
            };
            let cmap = ServerCmap {
                format: SERVER_CMAP_MAGIC.into(),
                version: SERVER_CMAP_VERSION,
                artifact_id: artifact_id.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                mappings: result.mappings,
            };
            match encode_server_cmap(&cmap) {
                Ok(bytes) => bytes,
                Err(_) => {
                    let _ = self
                        .store
                        .mark_preview_file_failed(
                            &job.preview_id,
                            &job.file_id,
                            "MAPPING_ENCODE_FAILED",
                            "Mapping encoding failed",
                        )
                        .await;
                    return;
                }
            }
        };

        if self
            .store
            .write_preview_file_ready(
                &job.preview_id,
                &job.file_id,
                result.markdown.as_bytes(),
                &mapping_bytes,
                result.masked_entity_count,
                &artifact_id,
            )
            .await
            .is_err()
        {
            let _ = self
                .store
                .mark_preview_file_failed(
                    &job.preview_id,
                    &job.file_id,
                    "OUTPUT_WRITE_FAILED",
                    "Output could not be stored",
                )
                .await;
        }
    }
}
