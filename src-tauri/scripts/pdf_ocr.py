#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Structured PDF text extraction + OCR.

Outputs a JSON document (schema version "1.0") to stdout.
Stderr is used for progress / diagnostic messages only.

CLI arguments:
  <pdf_path>                  Input PDF path (required).
  --start-page N              1-indexed first page (optional).
  --end-page N                1-indexed last page (optional).
  --model-dir DIR             EasyOCR model storage directory (optional).
  --max-pages N               Max pages to process (default 200).
  --max-pixels-per-page N     Max pixels per page at given DPI (default 12_000_000;
                               kept in sync with component-runtime's
                               OcrConfig::default() so a standard Letter/A4/Legal
                               full-page scan at 300 DPI is not rejected).
  --max-total-pixels N        Max total pixels across all selected pages.
  --dpi N                     Rendering DPI (default 300).

Exit codes:
  0 = success (JSON on stdout is valid)
  1 = input error (file not found, limit exceeded, page range invalid)
  2 = dependency error (PyMuPDF not installed)
  3 = OCR dependency error (EasyOCR/PIL not installed, model missing, download blocked)
  4 = OCR processing error (timeout, internal failure)

Output JSON schema: see docstring in component-runtime/src/lib.rs.
"""

import argparse
import json
import math
import os
import sys


def eprint(*args, **kwargs):
    print(*args, file=sys.stderr, **kwargs)


def emit_error(exit_code, message):
    """Output an empty result with the error field and exit with code."""
    result = {
        "schema_version": "1.0",
        "pages": [],
        "quality": {
            "total_pages": 0,
            "empty_pages": 0,
            "failed_pages": 0,
            "avg_confidence": 0.0,
        },
    }
    print(json.dumps(result, ensure_ascii=False))
    sys.exit(exit_code)


# ---------------------------------------------------------------------------
# Shared Reader builder (R2) — used by both self-test and OCR
# ---------------------------------------------------------------------------

def _build_reader(model_dir, download_enabled=False):
    """Build an EasyOCR Reader with explicit model directory and no download.

    Raises RuntimeError if Reader cannot be initialised offline.
    """
    import easyocr
    kwargs = {
        "lang_list": ["ch_sim", "en"],
        "gpu": False,
        "verbose": False,
        "download_enabled": download_enabled,
    }
    if model_dir:
        kwargs["model_storage_directory"] = model_dir
    return easyocr.Reader(**kwargs)


# ---------------------------------------------------------------------------
# PyMuPDF text extraction (text-layer PDFs)
# ---------------------------------------------------------------------------

def extract_text_pymupdf(doc, start_idx, end_idx):
    """Extract structured blocks from a text-layer PDF.

    Returns (pages, quality).  Every text block gets confidence=1.0,
    language="und".
    """
    import fitz

    pages = []
    total_confidence = 0.0
    confidence_count = 0
    empty_pages = 0
    failed_pages = 0

    for page_num in range(start_idx, end_idx):
        page = doc[page_num]
        rect = page.rect
        width = rect.width
        height = rect.height

        try:
            page_dict = page.get_text("dict")
            blocks = []
            for block in page_dict.get("blocks", []):
                if block.get("type") != 0:
                    continue
                lines_text = []
                for line in block.get("lines", []):
                    for span in line.get("spans", []):
                        text = span.get("text", "").strip()
                        if text:
                            lines_text.append(text)
                if not lines_text:
                    continue
                full_text = " ".join(lines_text)
                bbox_raw = block.get("bbox", [0, 0, 0, 0])
                blocks.append({
                    "text": full_text,
                    "bbox": bbox_raw[:4],
                    "confidence": 1.0,
                    "language": "und",
                })
                total_confidence += 1.0
                confidence_count += 1
        except Exception as exc:
            eprint(f"Page {page_num + 1} text extraction failed: {exc}")
            failed_pages += 1
            blocks = []

        if not blocks:
            empty_pages += 1

        pages.append({
            "page_number": page_num + 1,
            "width": width,
            "height": height,
            "blocks": blocks,
        })

    quality = {
        "total_pages": end_idx - start_idx,
        "empty_pages": empty_pages,
        "failed_pages": failed_pages,
        "avg_confidence": round(total_confidence / max(confidence_count, 1), 4),
    }
    return pages, quality


# ---------------------------------------------------------------------------
# EasyOCR (scanned PDFs)
# ---------------------------------------------------------------------------

def ocr_page_easyocr(reader, page, page_num, dpi):
    """OCR a single page with EasyOCR, returning blocks list."""
    from PIL import Image
    from io import BytesIO
    import numpy as np
    import fitz

    zoom = dpi / 72.0
    mat = fitz.Matrix(zoom, zoom)
    pix = page.get_pixmap(matrix=mat)
    img_data = pix.tobytes("png")
    img = Image.open(BytesIO(img_data))
    img_array = np.array(img)

    results = reader.readtext(img_array)
    blocks = []
    for detection in results:
        bbox_pts = detection[0]
        text = detection[1]
        confidence = detection[2]
        if not text.strip():
            continue
        xs = [p[0] / zoom for p in bbox_pts]
        ys = [p[1] / zoom for p in bbox_pts]
        pdf_bbox = [min(xs), min(ys), max(xs), max(ys)]
        blocks.append({
            "text": text.strip(),
            "bbox": pdf_bbox,
            "confidence": round(float(confidence), 4),
            "language": "und",
        })
    return blocks


def ocr_with_easyocr(doc, start_idx, end_idx, model_dir, dpi):
    """Run EasyOCR on a range of pages.

    Returns (pages, quality).
    """
    eprint("Initializing EasyOCR (model dir: {})".format(model_dir or "default"))

    try:
        reader = _build_reader(model_dir, download_enabled=False)
    except Exception as exc:
        raise RuntimeError(f"EasyOCR offline init failed: {exc}")

    pages = []
    total_confidence = 0.0
    confidence_count = 0
    empty_pages = 0
    failed_pages = 0

    for page_num in range(start_idx, end_idx):
        page = doc[page_num]
        rect = page.rect
        width = rect.width
        height = rect.height
        eprint(f"OCR page {page_num + 1}/{doc.page_count}...")
        try:
            blocks = ocr_page_easyocr(reader, page, page_num, dpi)
        except Exception as exc:
            eprint(f"Page {page_num + 1} OCR failed: {exc}")
            failed_pages += 1
            blocks = []

        for block in blocks:
            total_confidence += block["confidence"]
            confidence_count += 1
        if not blocks:
            empty_pages += 1

        pages.append({
            "page_number": page_num + 1,
            "width": width,
            "height": height,
            "blocks": blocks,
        })

    quality = {
        "total_pages": end_idx - start_idx,
        "empty_pages": empty_pages,
        "failed_pages": failed_pages,
        "avg_confidence": round(total_confidence / max(confidence_count, 1), 4),
    }
    return pages, quality


# ---------------------------------------------------------------------------
# Limit checking (R3)
# ---------------------------------------------------------------------------

def check_limits(
    doc, start_idx, end_idx, max_pages, max_pixels_per_page, max_total_pixels, dpi
):
    """Validate page/pixel limits BEFORE rendering.

    Returns None on success, or error string on failure.
    """
    total_pages = doc.page_count
    selected = end_idx - start_idx

    if max_pages > 0 and selected > max_pages:
        return f"Selected {selected} pages exceeds limit of {max_pages}"

    # At given DPI, each page pixel-area = (page_width * zoom) * (page_height * zoom)
    # where zoom = dpi / 72.
    zoom = dpi / 72.0
    for pn in range(start_idx, end_idx):
        page = doc[pn]
        w = page.rect.width
        h = page.rect.height
        pixels = int(math.ceil(w * zoom)) * int(math.ceil(h * zoom))
        if max_pixels_per_page > 0 and pixels > max_pixels_per_page:
            return (
                f"Page {pn + 1} at {dpi} DPI: {pixels:,} px exceeds "
                f"max {max_pixels_per_page:,} px per page"
            )

    if max_total_pixels > 0:
        total_px = 0
        for pn in range(start_idx, end_idx):
            page = doc[pn]
            w = page.rect.width
            h = page.rect.height
            total_px += int(math.ceil(w * zoom)) * int(math.ceil(h * zoom))
        if total_px > max_total_pixels:
            return (
                f"Total pixels across {selected} pages: {total_px:,} exceeds "
                f"max {max_total_pixels:,}"
            )

    return None


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Extract text or OCR from PDF, output JSON to stdout."
    )
    parser.add_argument("pdf_path", help="Path to the input PDF file")
    parser.add_argument("--start-page", type=int, default=None)
    parser.add_argument("--end-page", type=int, default=None)
    parser.add_argument("--model-dir", type=str, default=None)
    parser.add_argument("--max-pages", type=int, default=200)
    parser.add_argument("--max-pixels-per-page", type=int, default=12_000_000)
    parser.add_argument("--max-total-pixels", type=int, default=1_600_000_000)
    parser.add_argument("--dpi", type=int, default=300)
    args = parser.parse_args()

    pdf_path = args.pdf_path
    if not os.path.exists(pdf_path):
        eprint(f"File not found: {pdf_path}")
        emit_error(1, "File not found")

    try:
        import fitz
    except ImportError as exc:
        eprint(f"PyMuPDF import failed: {exc}")
        emit_error(2, f"PyMuPDF import failed: {exc}")

    try:
        doc = fitz.open(pdf_path)
    except Exception as exc:
        eprint(f"Cannot open PDF: {exc}")
        emit_error(1, f"Cannot open PDF: {exc}")

    total_pages = doc.page_count
    eprint(f"PDF has {total_pages} pages")

    start_idx = 0
    end_idx = total_pages
    if args.start_page is not None:
        start_idx = max(0, args.start_page - 1)
    if args.end_page is not None:
        end_idx = min(total_pages, args.end_page)
    if start_idx >= end_idx:
        doc.close()
        eprint("Invalid page range")
        emit_error(1, "Invalid page range")

    # ---- R3: page/pixel limit check BEFORE rendering ----
    limit_error = check_limits(
        doc,
        start_idx,
        end_idx,
        args.max_pages,
        args.max_pixels_per_page,
        args.max_total_pixels,
        args.dpi,
    )
    if limit_error:
        doc.close()
        eprint(f"LIMIT: {limit_error}")
        emit_error(1, limit_error)

    eprint(f"Processing pages {start_idx + 1}-{end_idx}")

    # Step 1: Try PyMuPDF text extraction
    eprint("Trying text-layer extraction...")
    pages, quality = extract_text_pymupdf(doc, start_idx, end_idx)

    has_text = any(
        bool(p["blocks"]) and any(b["text"].strip() for b in p["blocks"])
        for p in pages
    )

    if not has_text:
        eprint("No text layer detected; switching to OCR...")
        try:
            import easyocr
            import numpy as np
            from PIL import Image
        except ImportError as exc:
            doc.close()
            eprint(f"OCR dependencies not available: {exc}")
            # Exit 3: OCR dependency missing
            quality["failed_pages"] = quality["total_pages"]
            result = {
                "schema_version": "1.0",
                "pages": pages,
                "quality": quality,
            }
            print(json.dumps(result, ensure_ascii=False))
            sys.exit(3)

        try:
            ocr_pages, ocr_quality = ocr_with_easyocr(
                doc, start_idx, end_idx, args.model_dir, args.dpi
            )
            pages = ocr_pages
            quality = ocr_quality
        except Exception as exc:
            doc.close()
            eprint(f"OCR processing failed: {exc}")
            quality["failed_pages"] = quality["total_pages"]
            result = {
                "schema_version": "1.0",
                "pages": pages,
                "quality": quality,
            }
            print(json.dumps(result, ensure_ascii=False))
            sys.exit(4)

    doc.close()

    result = {
        "schema_version": "1.0",
        "pages": pages,
        "quality": quality,
    }
    print(json.dumps(result, ensure_ascii=False))


if __name__ == "__main__":
    main()
