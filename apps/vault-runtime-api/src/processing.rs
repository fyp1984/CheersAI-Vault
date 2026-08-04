//! Single shared processing pipeline.
//!
//! Both the direct-batch worker (`Runtime::process_job` in `lib.rs`) and the
//! preview worker (`preview.rs`) call [`process_input`] and only
//! [`process_input`]. This is the one place that runs legacy `.ppt`
//! conversion, parsing, the OCR fallback and masking for the enterprise
//! Runtime — extracted verbatim from the pre-existing `process_job` body so
//! batch and preview processing can never drift apart. `engine-core` and
//! `component-runtime` algorithms themselves are not touched; this module
//! only orchestrates the existing calls in one place.

use component_runtime::OcrConfig;
use engine_core::{
    get_builtin_rules, ocr_result_to_markdown, parse_input, sensitive_term_rules, InputFormat,
    MappingEntry, MaskingRequest, MaskingService, SensitiveTermDefinition,
};

use crate::legacy_powerpoint;

/// Prefix marking an opaque `rule_ids` entry as a frozen sensitive-term
/// snapshot rather than a real engine-core/enterprise rule id.
///
/// Why this lives inside the plain `rule_ids: &[String]` channel instead of
/// a dedicated `ProcessingInput` field: `preview.rs` (the preview worker,
/// outside this task's file whitelist) constructs `ProcessingInput` directly
/// and is never touched by this task, so `ProcessingInput`'s field set must
/// stay exactly as it already was. `store.rs` (whitelisted) embeds the
/// snapshot it captured at batch/preview creation time as one extra entry
/// in the `rule_ids` vector it already builds; this module — the single
/// place both workers route through — decodes it back out below. The
/// snapshot never reaches any public API response, DOM, log or event; it
/// only ever travels through this internal, same-process channel.
const SENSITIVE_TERMS_SNAPSHOT_RULE_PREFIX: &str = "__sensitive_terms_snapshot__:";

/// Wrap an already-serialized sensitive-term snapshot (a JSON array of
/// [`SensitiveTermDefinition`]) into the opaque `rule_ids` entry this module
/// decodes back out in [`process_input`]. Returns `None` for an empty
/// snapshot (`"[]"`) since there is nothing to add.
pub fn encode_sensitive_terms_snapshot_entry(snapshot_json: &str) -> Option<String> {
    if snapshot_json == "[]" {
        return None;
    }
    Some(format!(
        "{SENSITIVE_TERMS_SNAPSHOT_RULE_PREFIX}{snapshot_json}"
    ))
}

/// Extract the frozen sensitive-term snapshot (if any) from a claimed job's
/// `rule_ids`. A missing, malformed or absent entry decodes to an empty
/// snapshot rather than an error — the surrounding rule set still applies.
fn decode_sensitive_terms_snapshot(rule_ids: &[String]) -> Vec<SensitiveTermDefinition> {
    rule_ids
        .iter()
        .find_map(|id| id.strip_prefix(SENSITIVE_TERMS_SNAPSHOT_RULE_PREFIX))
        .and_then(|json| serde_json::from_str(json).ok())
        .unwrap_or_default()
}

/// Inputs required to process one file. `bytes` must already be the raw
/// upload bytes read from controlled storage by the caller.
pub struct ProcessingInput<'a> {
    pub bytes: &'a [u8],
    pub display_name: &'a str,
    pub input_format: InputFormat,
    pub rule_ids: &'a [String],
    pub ocr_config: Option<&'a OcrConfig>,
}

/// Successful processing output — the same shape `write_completed` and the
/// preview worker both consume to persist masked Markdown + mapping.
#[derive(Debug)]
pub struct ProcessingOutput {
    pub markdown: String,
    pub mappings: Vec<MappingEntry>,
    pub masked_entity_count: usize,
}

/// A stable `{code, message}` pair suitable for `mark_failed` / preview file
/// failure recording. Never carries original text, stack traces or paths.
#[derive(Debug)]
pub struct ProcessingFailure {
    pub code: String,
    pub message: String,
}

/// Returns `true` when `display_name` has a `.ppt` extension (case-insensitive).
/// Used to decide whether bytes should enter the legacy PowerPoint converter
/// vs. going directly to the existing PPTX parser.
pub fn is_legacy_ppt_extension(display_name: &str) -> bool {
    let basename = display_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(display_name);
    match basename.rfind('.') {
        Some(pos) => {
            let ext = &basename[pos + 1..];
            ext.eq_ignore_ascii_case("ppt")
        }
        None => false,
    }
}

/// Run legacy-.ppt conversion (if applicable), parse, fall back to OCR for
/// text-less PDFs, then mask. Callers are responsible for reading the input
/// bytes and for persisting the returned `ProcessingOutput`; this function
/// performs no I/O against the database or the filesystem beyond invoking
/// the OCR subprocess.
pub async fn process_input(
    input: ProcessingInput<'_>,
) -> Result<ProcessingOutput, ProcessingFailure> {
    let is_ppt_ext = is_legacy_ppt_extension(input.display_name);
    let processed_bytes = if input.input_format == InputFormat::Powerpoint
        && is_ppt_ext
        && legacy_powerpoint::looks_like_legacy_ppt(input.bytes)
    {
        match legacy_powerpoint::convert_ppt_to_pptx(input.bytes).await {
            Ok(converted) => converted,
            Err(convert_err) => {
                let app_err = convert_err.to_app_error();
                return Err(ProcessingFailure {
                    code: app_err.code,
                    message: app_err.message,
                });
            }
        }
    } else {
        input.bytes.to_vec()
    };

    let content = match parse_input(&processed_bytes, input.input_format) {
        Ok(parsed) => parsed.markdown,
        Err(error) => {
            if error.code == "OCR_COMPONENT_REQUIRED" && input.input_format == InputFormat::Pdf {
                match run_ocr_on_pdf(input.ocr_config, &processed_bytes).await {
                    Ok(md) => md,
                    Err(ocr_err) => {
                        return Err(ProcessingFailure {
                            code: ocr_err.error_code().to_string(),
                            message: ocr_err.to_string(),
                        });
                    }
                }
            } else {
                return Err(ProcessingFailure {
                    code: error.code,
                    message: error.message,
                });
            }
        }
    };

    let mut rules: Vec<_> = get_builtin_rules()
        .iter()
        .filter(|rule| input.rule_ids.iter().any(|rule_id| rule_id == &rule.id))
        .cloned()
        .map(|mut rule| {
            rule.enabled = true;
            rule
        })
        .collect();
    // Enabled sensitive terms frozen at batch/preview creation time (6.1);
    // both this worker and the preview worker route through here, so B1
    // holds without preview.rs needing any change.
    rules.extend(sensitive_term_rules(&decode_sensitive_terms_snapshot(
        input.rule_ids,
    )));

    let result = MaskingService::mask(MaskingRequest {
        input_format: input.input_format,
        content,
        rules,
        deterministic_findings: vec![],
    })
    .map_err(|_| ProcessingFailure {
        code: "MASKING_FAILED".to_string(),
        message: "Masking failed".to_string(),
    })?;

    Ok(ProcessingOutput {
        markdown: result.markdown,
        mappings: result.mappings,
        masked_entity_count: result.masked_entity_count,
    })
}

/// Run OCR on PDF bytes via the configured OCR runtime.
///
/// Returns `Ok(markdown)` on success, or `Err(ocr_error)` on failure. The
/// caller is responsible for turning that into a `ProcessingFailure`.
pub async fn run_ocr_on_pdf(
    ocr_config: Option<&OcrConfig>,
    pdf_bytes: &[u8],
) -> Result<String, component_runtime::OcrError> {
    let config = ocr_config.ok_or_else(|| {
        component_runtime::OcrError::ComponentUnavailable(
            "OCR runtime not configured (set VAULT_OCR_PYTHON, VAULT_OCR_SCRIPT)".into(),
        )
    })?;

    let ocr_result = component_runtime::run_ocr(config, pdf_bytes, None).await?;
    let markdown = ocr_result_to_markdown(&ocr_result);

    if markdown.trim().is_empty() {
        return Err(component_runtime::OcrError::NoText);
    }

    Ok(markdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_ppt_extension_detection_is_case_insensitive_and_path_safe() {
        assert!(is_legacy_ppt_extension("slides.ppt"));
        assert!(is_legacy_ppt_extension("slides.PPT"));
        assert!(is_legacy_ppt_extension("dir/slides.Ppt"));
        assert!(is_legacy_ppt_extension("dir\\slides.ppt"));
        assert!(!is_legacy_ppt_extension("slides.pptx"));
        assert!(!is_legacy_ppt_extension("slides"));
        assert!(!is_legacy_ppt_extension(""));
    }

    #[tokio::test]
    async fn process_input_masks_plain_text_through_the_shared_pipeline() {
        let rule_ids = vec!["phone".to_string()];
        let output = process_input(ProcessingInput {
            bytes: b"Call 13900000000",
            display_name: "note.txt",
            input_format: InputFormat::Text,
            rule_ids: &rule_ids,
            ocr_config: None,
        })
        .await
        .unwrap();
        assert_eq!(output.masked_entity_count, 1);
        assert!(output.markdown.contains("***PHONE***1"));
        assert!(!output.markdown.contains("13900000000"));
        assert_eq!(output.mappings.len(), 1);
        assert_eq!(output.mappings[0].original, "13900000000");
    }

    #[tokio::test]
    async fn process_input_reports_a_stable_failure_for_corrupted_input() {
        let rule_ids = vec!["phone".to_string()];
        let failure = process_input(ProcessingInput {
            bytes: b"%PDF-1.7\n",
            display_name: "disguised.md",
            input_format: InputFormat::Markdown,
            rule_ids: &rule_ids,
            ocr_config: None,
        })
        .await
        .unwrap_err();
        assert_eq!(failure.code, "INPUT_CORRUPTED");
    }

    #[test]
    fn sensitive_terms_snapshot_entry_round_trips_and_skips_empty() {
        assert_eq!(encode_sensitive_terms_snapshot_entry("[]"), None);

        let snapshot = vec![SensitiveTermDefinition {
            id: "t1".into(),
            term: "张三".into(),
            category: "姓名".into(),
            enabled: true,
        }];
        let json = serde_json::to_string(&snapshot).unwrap();
        let entry = encode_sensitive_terms_snapshot_entry(&json).expect("non-empty snapshot");
        assert!(entry.starts_with(SENSITIVE_TERMS_SNAPSHOT_RULE_PREFIX));

        let rule_ids = vec!["phone".to_string(), entry];
        let decoded = decode_sensitive_terms_snapshot(&rule_ids);
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn sensitive_terms_snapshot_decode_defaults_to_empty_when_absent_or_malformed() {
        assert!(decode_sensitive_terms_snapshot(&["phone".to_string()]).is_empty());
        assert!(decode_sensitive_terms_snapshot(&[format!(
            "{SENSITIVE_TERMS_SNAPSHOT_RULE_PREFIX}not-json"
        )])
        .is_empty());
    }

    /// B1/6.1: `use_sensitive_terms` alone (no snapshot entry) masks nothing
    /// extra — the snapshot, not the bare rule id, drives sensitive-term
    /// masking, matching 6.5's "no snapshot column ⇒ empty snapshot" default
    /// for batches/previews created before this migration.
    #[tokio::test]
    async fn process_input_ignores_bare_use_sensitive_terms_without_a_snapshot_entry() {
        let rule_ids = vec!["use_sensitive_terms".to_string()];
        let output = process_input(ProcessingInput {
            bytes: b"contact 13900000000",
            display_name: "note.txt",
            input_format: InputFormat::Text,
            rule_ids: &rule_ids,
            ocr_config: None,
        })
        .await
        .unwrap();
        assert_eq!(output.masked_entity_count, 0);
        assert!(output.markdown.contains("13900000000"));
    }

    /// B1/6.1: a real snapshot entry masks the enabled sensitive term
    /// alongside a normal builtin rule in the same pass.
    #[tokio::test]
    async fn process_input_masks_sensitive_terms_from_a_snapshot_entry() {
        let snapshot = vec![SensitiveTermDefinition {
            id: "abc".into(),
            term: "内部代号".into(),
            category: "机密".into(),
            enabled: true,
        }];
        let entry =
            encode_sensitive_terms_snapshot_entry(&serde_json::to_string(&snapshot).unwrap())
                .unwrap();
        let rule_ids = vec!["phone".to_string(), entry];
        let output = process_input(ProcessingInput {
            bytes: "项目 内部代号 联系人 13900000000".as_bytes(),
            display_name: "note.txt",
            input_format: InputFormat::Text,
            rule_ids: &rule_ids,
            ocr_config: None,
        })
        .await
        .unwrap();
        assert_eq!(output.masked_entity_count, 2);
        assert!(output.markdown.contains("[机密]"));
        assert!(output.markdown.contains("***PHONE***1"));
        assert!(!output.markdown.contains("内部代号"));
        assert!(!output.markdown.contains("13900000000"));
    }
}
