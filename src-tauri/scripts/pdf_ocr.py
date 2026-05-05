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
            # 检测到扫描版 PDF，尝试使用 EasyOCR
            print("=" * 60, file=sys.stderr)
            print("⚠ 检测到图片型 PDF（扫描版），启动 OCR 识别...", file=sys.stderr)
            print("=" * 60, file=sys.stderr)
            
            try:
                import easyocr
                import numpy as np
                from PIL import Image
                from io import BytesIO
                
                print("正在初始化 EasyOCR（首次使用会下载模型，约 50MB）...", file=sys.stderr)
                # 初始化 EasyOCR（支持中英文）
                reader = easyocr.Reader(['ch_sim', 'en'], gpu=False, verbose=False)
                
                # 重新打开 PDF 进行 OCR
                doc = fitz.open(pdf_path)
                ocr_texts = []
                
                for page_num in range(len(doc)):
                    print(f"OCR 识别第 {page_num + 1}/{len(doc)} 页...", file=sys.stderr)
                    page = doc[page_num]
                    
                    # 将 PDF 页面转换为图片
                    pix = page.get_pixmap(matrix=fitz.Matrix(2, 2))  # 2x 缩放提高质量
                    img_data = pix.tobytes("png")
                    
                    # 转换为 PIL Image
                    img = Image.open(BytesIO(img_data))
                    img_array = np.array(img)
                    
                    # 使用 EasyOCR 识别
                    result = reader.readtext(img_array)
                    
                    # 提取文本
                    page_text = []
                    for detection in result:
                        # detection 格式: (bbox, text, confidence)
                        text = detection[1]
                        page_text.append(text)
                    
                    page_ocr_text = '\n'.join(page_text)
                    ocr_texts.append(page_ocr_text)
                    print(f"第 {page_num + 1} 页识别出 {len(page_ocr_text)} 个字符", file=sys.stderr)
                
                doc.close()
                
                # 合并所有页面的 OCR 结果
                final_text = '\n\n'.join([text for text in ocr_texts if text])
                print(f"OCR 总共识别: {len(final_text)} 个字符", file=sys.stderr)
                print("=" * 60, file=sys.stderr)
                
                if not final_text.strip():
                    raise RuntimeError("OCR 识别失败，未能提取到文本")
                
            except ImportError as ie:
                print("=" * 60, file=sys.stderr)
                print("❌ 缺少 EasyOCR 依赖", file=sys.stderr)
                print("=" * 60, file=sys.stderr)
                print("", file=sys.stderr)
                print("当前使用的是轻量版 OCR，仅支持文本型 PDF。", file=sys.stderr)
                print("要处理扫描版 PDF，需要安装完整版 OCR。", file=sys.stderr)
                print("", file=sys.stderr)
                print("解决方案：", file=sys.stderr)
                print("1. 进入「增强服务」页面", file=sys.stderr)
                print("2. 找到「OCR 文字识别服务」", file=sys.stderr)
                print("3. 点击「完全卸载」", file=sys.stderr)
                print("4. 重新点击「一键安装」（会安装完整版）", file=sys.stderr)
                print("", file=sys.stderr)
                print("完整版 OCR 包含 EasyOCR，可以识别图片中的文字。", file=sys.stderr)
                print("=" * 60, file=sys.stderr)
                raise RuntimeError("需要完整版 OCR 才能处理扫描版 PDF，请按照上述步骤安装")

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
