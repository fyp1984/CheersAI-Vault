#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
PDF OCR Script using EasyOCR and PyMuPDF
Extracts text from scanned PDF files using OCR
No external dependencies required (Tesseract, Poppler)
"""

import sys
import os
import io

try:
    import easyocr
    import fitz  # PyMuPDF
    import numpy as np
    from PIL import Image
except ImportError as e:
    print(f"ERROR: Missing required Python package: {e}", file=sys.stderr)
    print("Please install required packages:", file=sys.stderr)
    print("  pip install easyocr PyMuPDF", file=sys.stderr)
    sys.exit(1)

# 全局 OCR reader（避免重复初始化）
reader = None

def get_ocr_reader():
    """获取或初始化 OCR reader"""
    global reader
    if reader is None:
        print("Initializing EasyOCR (first time may download models)...", file=sys.stderr)
        # 支持中文简体和英文
        reader = easyocr.Reader(['ch_sim', 'en'], gpu=False)
        print("EasyOCR initialized successfully", file=sys.stderr)
    return reader

def extract_text_from_pdf(pdf_path):
    """
    Extract text from PDF using OCR
    
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
        
        # 获取 OCR reader
        ocr = get_ocr_reader()
        
        all_text = []
        
        # 处理每一页
        for page_num in range(len(doc)):
            print(f"Processing page {page_num + 1}/{len(doc)}...", file=sys.stderr)
            
            page = doc[page_num]
            
            # 将页面渲染为图片（高分辨率提高识别率）
            mat = fitz.Matrix(2.0, 2.0)  # 2x zoom for better quality
            pix = page.get_pixmap(matrix=mat)
            
            # 转换为 PIL Image
            img_data = pix.tobytes("png")
            img = Image.open(io.BytesIO(img_data))
            
            # 转换为 numpy array（EasyOCR 需要）
            img_array = np.array(img)
            
            # 执行 OCR
            print(f"Running OCR on page {page_num + 1}...", file=sys.stderr)
            result = ocr.readtext(img_array, detail=0, paragraph=True)
            
            # 合并识别结果
            page_text = '\n'.join(result)
            all_text.append(page_text)
            
            print(f"Page {page_num + 1} extracted {len(page_text)} characters", file=sys.stderr)
        
        doc.close()
        
        # 合并所有页面
        final_text = '\n\n'.join(all_text)
        print(f"Total extracted: {len(final_text)} characters", file=sys.stderr)
        
        return final_text
        
    except Exception as e:
        print(f"ERROR: OCR processing failed: {e}", file=sys.stderr)
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
