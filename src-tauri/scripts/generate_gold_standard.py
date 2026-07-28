#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Generate fictional image-only scan-PDF fixtures for OCR gold-standard testing.

This script is the SINGLE SOURCE OF TRUTH for both
`pdf_ocr_gold_standard.pdf` / `pdf_ocr_gold_standard.json` and
`pdf_ocr_blank_only.pdf`. The committed JSON must be exactly reproducible by
running this script with no arguments (verify with `--check`); it must never
be hand-edited.

Gold-standard fixture (3 pages):
  Page 1: Clear, high-contrast content — phone/email/id_card ("required
          clear fields") that MUST be detected, masked, counted, rendered
          masked in the Markdown artifact, and precisely restored.
  Page 2: Low-contrast (faint gray) content — phone/email/name ("supplementary
          low-quality fields"). Detection here is NOT guaranteed and is
          reported separately; it never contributes to expected_masked_count.
  Page 3: Blank.

Blank-only fixture (1 page, `--blank-only`):
  A single blank page with no text and no readable content at all, used to
  exercise the "no text detected by OCR" failure mode (OCR_NO_TEXT).

Every page of every fixture is rendered as a high-resolution bitmap (PIL
Image, generated at 300 DPI-equivalent pixel density for the physical page
size) and embedded into the PDF via PyMuPDF's `insert_image`. The resulting
PDF has ZERO text-layer objects: it is a genuine image-only/scanned PDF.

Self-verification happens twice:
  1. In-memory, right after each page is composed (`page.get_text()` must be
     empty), matching the original script's behaviour.
  2. After `doc.save()`, by re-opening the saved file from disk and checking
     page count, empty text layer, and at least one image object per page
     (see `verify_saved_pdf`). Either failing aborts with a non-zero exit
     and no fixture files are left on disk.

Font handling: the TrueType font used to render Chinese + Latin text is
REQUIRED. There is no silent fallback to PIL's built-in bitmap font (which
cannot render Chinese glyphs and produces useless fixtures). If the font
cannot be loaded, the script prints an error to stderr and exits with
status 1 without writing any file. The font path can be overridden via the
`GOLD_STANDARD_FONT_PATH` environment variable, e.g. to construct a negative
test case with a non-existent path.
"""

import argparse
import io
import json
import os
import shutil
import sys
import tempfile

FIXTURES_DIR = os.path.join(
    os.path.dirname(__file__) or ".",
    "../../apps/vault-runtime-api/tests/fixtures",
)

GOLD_PDF_NAME = "pdf_ocr_gold_standard.pdf"
GOLD_JSON_NAME = "pdf_ocr_gold_standard.json"
BLANK_ONLY_PDF_NAME = "pdf_ocr_blank_only.pdf"

# macOS Chinese-capable font — available on all Macs with Chinese language
# support. Overridable via GOLD_STANDARD_FONT_PATH to construct the G1
# negative test (point it at a path that does not exist).
FONT_PATH = os.environ.get(
    "GOLD_STANDARD_FONT_PATH", "/System/Library/Fonts/STHeiti Medium.ttc"
)

# Physical page size in PDF points (US Letter, 72 points/inch).
PAGE_WIDTH_PT = 612
PAGE_HEIGHT_PT = 792

# Render the source bitmaps at 300 DPI-equivalent pixel density so that when
# pdf_ocr.py rasterises the page at its own default 300 DPI, the embedded
# image is native resolution rather than an upscaled blur. This materially
# improves OCR recognition accuracy for the required clear fields.
RENDER_DPI = 300
SCALE = RENDER_DPI / 72.0
IMG_WIDTH = round(PAGE_WIDTH_PT * SCALE)
IMG_HEIGHT = round(PAGE_HEIGHT_PT * SCALE)

# Font sizes are specified in "points" (72/inch) and scaled to the render
# resolution, same as text position coordinates.
FONT_SIZE_PT = 30
LARGE_FONT_SIZE_PT = 40


def px(pt):
    """Convert a point-space coordinate/size to the render pixel space."""
    return round(pt * SCALE)


def eprint(*args, **kwargs):
    print(*args, file=sys.stderr, **kwargs)


def load_font_or_exit(size_pt):
    """Load the required TrueType font at the given point size.

    No fallback to ImageFont.load_default(): that bitmap font cannot render
    Chinese glyphs and would silently produce a useless (boxes-only) fixture.
    Exits the process with status 1 and writes nothing if the font is
    unavailable.
    """
    from PIL import ImageFont

    try:
        return ImageFont.truetype(FONT_PATH, px(size_pt))
    except (IOError, OSError) as exc:
        eprint(f"ERROR: required font not available: {FONT_PATH} ({exc})")
        eprint(
            "Set GOLD_STANDARD_FONT_PATH to a valid TrueType font capable of "
            "rendering Chinese glyphs."
        )
        sys.exit(1)


def render_pil_page(draw_func, bg="white"):
    """Create a PIL RGBA image at render resolution, paint it, return PNG bytes."""
    from PIL import Image, ImageDraw

    img = Image.new("RGBA", (IMG_WIDTH, IMG_HEIGHT), bg)
    draw = ImageDraw.Draw(img)
    draw_func(draw)
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return buf.getvalue()


def page_1_content(draw, font, lfont):
    """Clear content page — large, high-contrast, high-resolution text."""
    draw.text((px(50), px(80)), "项目合同确认书", fill="black", font=lfont)
    draw.text((px(50), px(150)), "甲方：张三", fill="black", font=font)
    draw.text((px(50), px(210)), "联系电话：13912345678", fill="black", font=font)
    # An ASCII "Email:" label (rather than the Chinese "电子邮箱：") avoids a
    # reproducible EasyOCR case-mangling quirk where a lowercase Latin value
    # immediately following Chinese label glyphs gets its first letter
    # recognised as uppercase (see result doc for raw-output evidence).
    draw.text((px(50), px(270)), "Email: zhangsan@example.cn", fill="black", font=font)
    draw.text((px(50), px(330)), "身份证号：110101199001011234", fill="black", font=font)


def page_2_content(draw, font, lfont):
    """Low contrast page — gray text, deliberately harder for OCR."""
    gray = (153, 153, 153)  # ~60% gray
    draw.text((px(50), px(100)), "补充条款", fill=gray, font=font)
    draw.text((px(50), px(160)), "联系人：李四", fill=gray, font=font)
    draw.text((px(50), px(220)), "电话：13800138000", fill=gray, font=font)
    draw.text((px(50), px(280)), "邮箱：lisi@test.com", fill=gray, font=font)


REQUIRED_CLEAR_FIELDS = [
    {"text": "13912345678", "category": "phone", "page": 1},
    {"text": "zhangsan@example.cn", "category": "email", "page": 1},
    {"text": "110101199001011234", "category": "id_card", "page": 1},
]

SUPPLEMENTARY_LOW_QUALITY_FIELDS = [
    {"text": "13800138000", "category": "phone", "page": 2},
    {"text": "lisi@test.com", "category": "email", "page": 2},
    {"text": "李四", "category": "chinese_name", "page": 2},
    {"text": "补充条款", "category": "title", "page": 2},
]

FIELDS_NOT_MASKED_BY_CURRENT_RULES = ["张三", "李四", "项目合同确认书", "补充条款"]


def build_gold_dict():
    """Build the gold-standard JSON contract as a plain dict (no I/O)."""
    return {
        "fixture": GOLD_PDF_NAME,
        "description": (
            "3-page fictional image-only scan-PDF for OCR gold-standard "
            "testing"
        ),
        "pages": 3,
        "has_text_layer": False,
        "generated_with": "PIL+PyMuPDF insert_image (no PDF text objects)",
        "generation_font": {
            "path": FONT_PATH,
            "size_pt": FONT_SIZE_PT,
            "large_size_pt": LARGE_FONT_SIZE_PT,
            "render_dpi": RENDER_DPI,
        },
        "expected_fields": {
            "page_1": [
                {"text": "项目合同确认书", "category": "title"},
                {"text": "张三", "category": "chinese_name"},
                {"text": "13912345678", "category": "phone"},
                {"text": "zhangsan@example.cn", "category": "email"},
                {"text": "110101199001011234", "category": "id_card"},
            ],
            "page_2": [
                {"text": "补充条款", "category": "title"},
                {"text": "李四", "category": "chinese_name"},
                {"text": "13800138000", "category": "phone"},
                {"text": "lisi@test.com", "category": "email"},
            ],
            "page_3": [],
        },
        # Two mutually exclusive sets. required_clear_fields are the only
        # fields that count toward expected_masked_count: they MUST be
        # detected, masked, counted, rendered masked in the Markdown
        # artifact, and precisely restored. supplementary_low_quality_fields
        # come from the deliberately low-contrast page 2 and are reported
        # separately; their detection is not guaranteed and never affects
        # expected_masked_count.
        "required_clear_fields": REQUIRED_CLEAR_FIELDS,
        "supplementary_low_quality_fields": SUPPLEMENTARY_LOW_QUALITY_FIELDS,
        "masking_expected": {
            "rules_enabled": ["phone", "email", "id_card"],
            "expected_masked_count": len(REQUIRED_CLEAR_FIELDS),
            "fields_not_masked_by_current_rules": FIELDS_NOT_MASKED_BY_CURRENT_RULES,
        },
    }


def verify_no_text_layer(doc):
    """In-memory self-check: every page must report empty get_text()."""
    for i in range(len(doc)):
        text = doc[i].get_text("text").strip()
        if text:
            eprint(
                f"FAIL: page {i + 1} has extractable text layer: {text[:120]!r}"
            )
            sys.exit(1)


def verify_saved_pdf(pdf_path, expected_pages):
    """Re-open the PDF from disk and verify page count, empty text layer,
    and at least one image object per page. Exits non-zero on any failure.
    """
    import fitz

    doc = fitz.open(pdf_path)
    try:
        if len(doc) != expected_pages:
            eprint(
                f"FAIL: saved PDF has {len(doc)} pages, expected {expected_pages}"
            )
            sys.exit(1)
        for i in range(len(doc)):
            page = doc[i]
            text = page.get_text("text").strip()
            if text:
                eprint(
                    f"FAIL: saved PDF page {i + 1} has extractable text: {text[:120]!r}"
                )
                sys.exit(1)
            images = page.get_images(full=True)
            if len(images) < 1:
                eprint(f"FAIL: saved PDF page {i + 1} has no image object")
                sys.exit(1)
            eprint(f"  saved page {i + 1}: no text layer, {len(images)} image object(s) ✅")
    finally:
        doc.close()


def generate_gold_fixture(output_dir):
    """Generate the gold-standard PDF + JSON into output_dir.

    Returns (pdf_path, gold_json_path, gold_dict).
    """
    import fitz

    os.makedirs(output_dir, exist_ok=True)
    pdf_path = os.path.join(output_dir, GOLD_PDF_NAME)
    gold_path = os.path.join(output_dir, GOLD_JSON_NAME)

    font = load_font_or_exit(FONT_SIZE_PT)
    lfont = load_font_or_exit(LARGE_FONT_SIZE_PT)

    doc = fitz.open()

    page1_png = render_pil_page(lambda draw: page_1_content(draw, font, lfont))
    page = doc.new_page(width=PAGE_WIDTH_PT, height=PAGE_HEIGHT_PT)
    page.insert_image(page.rect, stream=page1_png)

    page2_png = render_pil_page(lambda draw: page_2_content(draw, font, lfont))
    page = doc.new_page(width=PAGE_WIDTH_PT, height=PAGE_HEIGHT_PT)
    page.insert_image(page.rect, stream=page2_png)

    blank_png = render_pil_page(lambda draw: None)
    page = doc.new_page(width=PAGE_WIDTH_PT, height=PAGE_HEIGHT_PT)
    page.insert_image(page.rect, stream=blank_png)

    verify_no_text_layer(doc)

    doc.save(pdf_path, garbage=4, deflate=True)
    doc.close()

    verify_saved_pdf(pdf_path, expected_pages=3)

    gold = build_gold_dict()
    with open(gold_path, "w", encoding="utf-8") as f:
        json.dump(gold, f, ensure_ascii=False, indent=2)
        f.write("\n")

    return pdf_path, gold_path, gold


def generate_blank_only_fixture(output_dir):
    """Generate the single-page blank/no-text PDF fixture used for the
    'no text detected by OCR' failure mode (G7c). Returns the PDF path.
    """
    import fitz

    os.makedirs(output_dir, exist_ok=True)
    pdf_path = os.path.join(output_dir, BLANK_ONLY_PDF_NAME)

    doc = fitz.open()
    blank_png = render_pil_page(lambda draw: None)
    page = doc.new_page(width=PAGE_WIDTH_PT, height=PAGE_HEIGHT_PT)
    page.insert_image(page.rect, stream=blank_png)

    verify_no_text_layer(doc)

    doc.save(pdf_path, garbage=4, deflate=True)
    doc.close()

    verify_saved_pdf(pdf_path, expected_pages=1)

    return pdf_path


def run_check_mode():
    """--check: regenerate the gold-standard JSON into a temp directory and
    compare it byte-for-byte against the already-committed JSON. The
    committed JSON must never be hand-edited; this is the enforcement
    mechanism. Exits non-zero on any mismatch.
    """
    committed_gold_path = os.path.join(FIXTURES_DIR, GOLD_JSON_NAME)
    if not os.path.exists(committed_gold_path):
        eprint(f"FAIL: committed JSON not found: {committed_gold_path}")
        sys.exit(1)

    tmp_dir = tempfile.mkdtemp(prefix="gold_standard_check_")
    try:
        _, tmp_gold_path, _ = generate_gold_fixture(tmp_dir)
        with open(tmp_gold_path, "rb") as f:
            regenerated_bytes = f.read()
        with open(committed_gold_path, "rb") as f:
            committed_bytes = f.read()

        if regenerated_bytes != committed_bytes:
            eprint("FAIL: --check mismatch between regenerated and committed JSON")
            eprint(f"  committed:    {committed_gold_path} ({len(committed_bytes)} bytes)")
            eprint(f"  regenerated:  {tmp_gold_path} ({len(regenerated_bytes)} bytes)")
            for i, (a, b) in enumerate(zip(committed_bytes, regenerated_bytes)):
                if a != b:
                    eprint(f"  first differing byte at offset {i}")
                    break
            sys.exit(1)

        print(
            "--check PASS: committed JSON is byte-for-byte reproducible by "
            "this script"
        )
    finally:
        shutil.rmtree(tmp_dir, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(
        description=(
            "Generate the OCR gold-standard fixtures. With no flags, "
            "(re)generates pdf_ocr_gold_standard.pdf/.json in the fixtures "
            "directory."
        )
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Verify the committed JSON is byte-for-byte reproducible; do not overwrite it.",
    )
    parser.add_argument(
        "--blank-only",
        action="store_true",
        help="Generate pdf_ocr_blank_only.pdf (single blank page, no text) instead of the gold standard.",
    )
    args = parser.parse_args()

    try:
        import fitz  # noqa: F401
    except ImportError:
        eprint("ERROR: PyMuPDF (fitz) is required. pip install PyMuPDF")
        sys.exit(1)

    if args.check:
        run_check_mode()
        return

    if args.blank_only:
        # Font is not used by the blank fixture, but validate anyway so a
        # broken font environment fails loudly and consistently.
        load_font_or_exit(FONT_SIZE_PT)
        pdf_path = generate_blank_only_fixture(FIXTURES_DIR)
        print(f"Created: {pdf_path}")
        print(f"Size: {os.path.getsize(pdf_path)} bytes")
        print("Self-verification: PASS (no text layer, 1 page)")
        return

    pdf_path, gold_path, gold = generate_gold_fixture(FIXTURES_DIR)
    print(f"Created: {pdf_path}")
    print(f"Created: {gold_path}")
    print(f"Size: {os.path.getsize(pdf_path)} bytes")
    print(
        f"Self-verification: PASS (no text layer, image object present, "
        f"{gold['pages']} pages)"
    )


if __name__ == "__main__":
    main()
