#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
PDF text extraction helper based on PyMuPDF.
Used as the lightweight cross-platform fallback runtime for Vault.
"""

import sys
import os
try:
    import fitz  # PyMuPDF
except ImportError as e:
    print(f"ERROR: Missing required Python package: {e}", file=sys.stderr)
    print("Please install required packages:", file=sys.stderr)
    print("  pip install PyMuPDF", file=sys.stderr)
    sys.exit(1)

def extract_text_from_pdf(pdf_path):
    """
    Extract text from PDF using PyMuPDF
    
    Args:
        pdf_path: Path to the PDF file
        
    Returns:
        Extracted text as string
    """
    try:
        # 打开 PDF
        print(f"Opening PDF: {pdf_path}", file=sys.stderr)
        doc = fitz.open(pdf_path)
        print(f"PDF has {len(doc)} pages", file=sys.stderr)

        all_text = []

        # 处理每一页并直接提取文本
        for page_num in range(len(doc)):
            print(f"Processing page {page_num + 1}/{len(doc)}...", file=sys.stderr)
            page = doc[page_num]

            page_text = page.get_text("text").strip()
            all_text.append(page_text)

            print(f"Page {page_num + 1} extracted {len(page_text)} characters", file=sys.stderr)

        doc.close()

        # 合并所有页面
        final_text = '\n\n'.join([text for text in all_text if text])
        print(f"Total extracted: {len(final_text)} characters", file=sys.stderr)

        if not final_text.strip():
            raise RuntimeError("未提取到可用文本，当前轻量运行时不支持图片型 OCR")

        return final_text

    except Exception as e:
        print(f"ERROR: PDF text extraction failed: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc(file=sys.stderr)
        raise

def main():
    if len(sys.argv) != 2:
        print("Usage: python pdf_ocr.py <pdf_file_path>", file=sys.stderr)
        sys.exit(1)
    
    pdf_path = sys.argv[1]
    
    if not os.path.exists(pdf_path):
        print(f"ERROR: File not found: {pdf_path}", file=sys.stderr)
        sys.exit(1)
    
    try:
        text = extract_text_from_pdf(pdf_path)
        
        # Output the extracted text to stdout
        print(text)
        
    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    main()
