//! Unified OCR document model.
//!
//! This module lives in engine-core because both the desktop and enterprise
//! runtimes consume the same data types and the same markdown-conversion
//! function. It does NOT depend on Python, subprocesses, Tauri, or any
//! I/O — it is pure data transformation once the OCR provider has returned
//! a structured result.
//!
//! Architecture boundary
//! ---------------------
//! - engine-core defines the model (`OcrResult`, `OcrPage`, `OcrTextBlock`)
//!   and the stateless `ocr_result_to_markdown()` conversion.
//! - The OCR *execution* (subprocess, timeouts, path discovery) lives in the
//!   shared `component-runtime` crate, not here.
//! - Masking is unchanged: the markdown output from `ocr_result_to_markdown`
//!   enters the same `MaskingService` as any other format.

use serde::{Deserialize, Serialize};

/// A single recognised text block (word / phrase / line).
///
/// Coordinates are in PDF points (1/72 inch), using the standard PDF
/// coordinate space: origin at bottom-left, Y increasing upward.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrTextBlock {
    /// Recognised text content (may be empty for blank / whitespace-only
    /// blocks — those should be filtered before reaching this struct).
    pub text: String,

    /// Bounding box `[x0, y0, x1, y1]` in PDF points.
    pub bbox: [f64; 4],

    /// Recognition confidence in `[0.0, 1.0]`.  For text-layer extractions
    /// (PyMuPDF) this is always 1.0; for OCR engines it is the reported
    /// confidence.  Values outside `[0.0, 1.0]` are rejected at parse time.
    pub confidence: f64,

    /// BCP-47 language tag, e.g. `"zh"`, `"en"`, or `"und"` when the
    /// language cannot be reliably determined.
    pub language: String,
}

/// A single page's OCR output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrPage {
    /// 1-indexed page number within the document.
    pub page_number: usize,

    /// Text blocks in reading order (top-to-bottom, left-to-right).
    pub blocks: Vec<OcrTextBlock>,

    /// Page width in PDF points.
    pub width: f64,

    /// Page height in PDF points.
    pub height: f64,
}

/// Quality metadata for an OCR run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrQualitySummary {
    /// Total pages processed.
    pub total_pages: usize,

    /// Pages where no text block had non-empty text.
    pub empty_pages: usize,

    /// Pages that could not be processed.
    pub failed_pages: usize,

    /// Mean confidence across all non-empty text blocks, or `1.0` if no
    /// text layer OCR was needed.
    pub avg_confidence: f64,
}

/// Full structured OCR result, direct from the Python provider.
///
/// This is what gets written to stdout by `pdf_ocr.py` (as JSON) and
/// what the `component-runtime` crate deserialises into.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrResult {
    /// Schema version for forward compatibility, e.g. `"1.0"`.
    pub schema_version: String,

    /// Pages in document order.
    pub pages: Vec<OcrPage>,

    /// Aggregate quality information.
    pub quality: OcrQualitySummary,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate that an `OcrResult` is structurally sound.
///
/// Returns the first validation error as a human-readable message, or `None`
/// if the result passes all checks.
pub fn validate_ocr_result(result: &OcrResult) -> Option<String> {
    if result.pages.is_empty() {
        return Some("OCR result contains no pages".into());
    }

    // R7: quality.total_pages must match actual pages
    if result.quality.total_pages != result.pages.len() {
        return Some(format!(
            "quality.total_pages ({}) != number of pages ({})",
            result.quality.total_pages,
            result.pages.len()
        ));
    }

    // R7: any failed page → reject whole file
    if result.quality.failed_pages > 0 {
        return Some(format!(
            "{} page(s) failed; complete result rejected",
            result.quality.failed_pages
        ));
    }

    // R7: empty_pages must not exceed total_pages
    if result.quality.empty_pages > result.quality.total_pages {
        return Some(format!(
            "empty_pages ({}) exceeds total_pages ({})",
            result.quality.empty_pages, result.quality.total_pages
        ));
    }

    // R7: avg_confidence must be in [0, 1]
    if result.quality.avg_confidence.is_nan()
        || result.quality.avg_confidence.is_infinite()
        || !(0.0..=1.0).contains(&result.quality.avg_confidence)
    {
        return Some(format!(
            "avg_confidence {} is not in [0.0, 1.0]",
            result.quality.avg_confidence
        ));
    }

    let mut prev_page: Option<usize> = None;
    for page in &result.pages {
        let pn = page.page_number;
        if pn == 0 {
            return Some("page_number must be 1-indexed".into());
        }
        if let Some(prev) = prev_page {
            if pn <= prev {
                return Some(format!("pages out of order: {} after {}", pn, prev));
            }
        }
        prev_page = Some(pn);

        for (bi, block) in page.blocks.iter().enumerate() {
            if block.confidence.is_nan()
                || block.confidence.is_infinite()
                || !(0.0..=1.0).contains(&block.confidence)
            {
                return Some(format!(
                    "page {} block {}: confidence {} is not in [0.0, 1.0]",
                    pn, bi, block.confidence
                ));
            }
        }
    }

    if result.quality.total_pages == 0 {
        return Some("quality.total_pages must be > 0".into());
    }

    None
}

// ---------------------------------------------------------------------------
// Markdown conversion
// ---------------------------------------------------------------------------

/// Convert a validated `OcrResult` into Markdown suitable for the masking
/// pipeline.
///
/// The output is organised by page:
/// ```markdown
/// ## Page 1
///
/// text from blocks on page 1
///
/// ## Page 2
///
/// text from blocks on page 2
/// ```
///
/// Blank / whitespace-only blocks are silently skipped.
/// Low-confidence blocks (confidence < 0.5) are included but annotated in
/// the quality summary.
pub fn ocr_result_to_markdown(result: &OcrResult) -> String {
    let mut md = String::new();
    let mut has_any_text = false;

    for page in &result.pages {
        // Collect non-empty text from blocks in order.
        let lines: Vec<&str> = page
            .blocks
            .iter()
            .filter(|b| !b.text.trim().is_empty())
            .map(|b| b.text.as_str())
            .collect();

        if lines.is_empty() {
            // Still emit a heading for the page even if it has no text.
            if !md.is_empty() {
                md.push('\n');
            }
            md.push_str(&format!("## Page {}\n\n", page.page_number));
            continue;
        }

        if !md.is_empty() {
            md.push('\n');
        }
        md.push_str(&format!("## Page {}\n\n", page.page_number));

        // Join blocks with newlines to preserve line-level structure.
        // Multi-line blocks keep their internal newlines.
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                md.push('\n');
            }
            md.push_str(line);
        }

        has_any_text = true;
    }

    if !has_any_text && !result.pages.is_empty() {
        // Every page was empty — return something rather than silence.
        // The empty-page count in the quality summary already captures this.
        if md.is_empty() {
            md.push_str(&format!("## Page {}\n\n", result.pages[0].page_number));
        }
    }

    md
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_result() -> OcrResult {
        OcrResult {
            schema_version: "1.0".into(),
            pages: vec![
                OcrPage {
                    page_number: 1,
                    blocks: vec![
                        OcrTextBlock {
                            text: "Hello World".into(),
                            bbox: [10.0, 10.0, 200.0, 30.0],
                            confidence: 0.95,
                            language: "en".into(),
                        },
                        OcrTextBlock {
                            text: "项目启动会议".into(),
                            bbox: [10.0, 50.0, 300.0, 70.0],
                            confidence: 0.88,
                            language: "zh".into(),
                        },
                    ],
                    width: 612.0,
                    height: 792.0,
                },
                OcrPage {
                    page_number: 2,
                    blocks: vec![OcrTextBlock {
                        text: "Contact: 13900000000".into(),
                        bbox: [10.0, 10.0, 250.0, 30.0],
                        confidence: 0.92,
                        language: "en".into(),
                    }],
                    width: 612.0,
                    height: 792.0,
                },
            ],
            quality: OcrQualitySummary {
                total_pages: 2,
                empty_pages: 0,
                failed_pages: 0,
                avg_confidence: 0.9167,
            },
        }
    }

    #[test]
    fn valid_result_passes_validation() {
        let result = sample_result();
        assert!(validate_ocr_result(&result).is_none());
    }

    #[test]
    fn empty_pages_rejected() {
        let result = OcrResult {
            pages: vec![],
            ..sample_result()
        };
        assert!(validate_ocr_result(&result).is_some());
    }

    #[test]
    fn failed_pages_rejected() {
        let mut result = sample_result();
        result.quality.failed_pages = 1;
        let err = validate_ocr_result(&result);
        assert!(err.is_some());
        assert!(err.unwrap().contains("failed"));
    }

    #[test]
    fn avg_confidence_out_of_range_rejected() {
        let mut result = sample_result();
        result.quality.avg_confidence = -0.1;
        assert!(validate_ocr_result(&result).is_some());
        result.quality.avg_confidence = 1.5;
        assert!(validate_ocr_result(&result).is_some());
    }

    #[test]
    fn total_pages_mismatch_rejected() {
        let mut result = sample_result();
        result.quality.total_pages = 99;
        let err = validate_ocr_result(&result);
        assert!(err.is_some());
        assert!(err.unwrap().contains("total_pages"));
    }

    #[test]
    fn empty_pages_exceeds_total_rejected() {
        let mut result = sample_result();
        result.quality.empty_pages = 99;
        let err = validate_ocr_result(&result);
        assert!(err.is_some());
        assert!(err.unwrap().contains("empty_pages"));
    }

    #[test]
    fn zero_indexed_page_rejected() {
        let mut result = sample_result();
        result.pages[0].page_number = 0;
        assert!(validate_ocr_result(&result).is_some());
    }

    #[test]
    fn out_of_order_pages_rejected() {
        let mut result = sample_result();
        result.pages[1].page_number = 1; // same as page 0
        assert!(validate_ocr_result(&result).is_some());
    }

    #[test]
    fn nan_confidence_rejected() {
        let mut result = sample_result();
        result.pages[0].blocks[0].confidence = f64::NAN;
        assert!(validate_ocr_result(&result).is_some());
    }

    #[test]
    fn negative_confidence_rejected() {
        let mut result = sample_result();
        result.pages[0].blocks[0].confidence = -0.1;
        assert!(validate_ocr_result(&result).is_some());
    }

    #[test]
    fn confidence_greater_than_one_rejected() {
        let mut result = sample_result();
        result.pages[0].blocks[0].confidence = 1.1;
        assert!(validate_ocr_result(&result).is_some());
    }

    #[test]
    fn markdown_has_correct_page_headings() {
        let result = sample_result();
        let md = ocr_result_to_markdown(&result);
        assert!(md.contains("## Page 1"), "md: {md}");
        assert!(md.contains("## Page 2"), "md: {md}");
        assert!(md.contains("Hello World"));
        assert!(md.contains("项目启动会议"));
        assert!(md.contains("13900000000"));
    }

    #[test]
    fn markdown_order_follows_reading_order() {
        let result = sample_result();
        let md = ocr_result_to_markdown(&result);
        let hello_pos = md.find("Hello World").unwrap();
        let meeting_pos = md.find("项目启动会议").unwrap();
        let phone_pos = md.find("13900000000").unwrap();
        assert!(hello_pos < meeting_pos, "reading order on page 1");
        assert!(meeting_pos < phone_pos, "page 1 before page 2");
    }

    #[test]
    fn empty_blocks_are_skipped() {
        let mut result = sample_result();
        result.pages[0].blocks.push(OcrTextBlock {
            text: "   ".into(),
            bbox: [0.0, 0.0, 10.0, 10.0],
            confidence: 0.5,
            language: "und".into(),
        });
        let md = ocr_result_to_markdown(&result);
        // The whitespace-only block should not add extra content
        assert_eq!(md.matches("Hello World").count(), 1);
    }

    #[test]
    fn blank_page_still_gets_heading() {
        let result = OcrResult {
            schema_version: "1.0".into(),
            pages: vec![OcrPage {
                page_number: 1,
                blocks: vec![],
                width: 612.0,
                height: 792.0,
            }],
            quality: OcrQualitySummary {
                total_pages: 1,
                empty_pages: 1,
                failed_pages: 0,
                avg_confidence: 1.0,
            },
        };
        let md = ocr_result_to_markdown(&result);
        assert!(md.contains("## Page 1"));
    }

    #[test]
    fn quality_summary_counts_are_reasonable() {
        let result = sample_result();
        assert_eq!(result.quality.total_pages, 2);
        assert_eq!(result.quality.empty_pages, 0);
        assert!(result.quality.avg_confidence > 0.9);
    }

    #[test]
    fn serde_round_trip() {
        let result = sample_result();
        let json = serde_json::to_string(&result).unwrap();
        let parsed: OcrResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, parsed);
    }
}
