@echo off
echo ========================================
echo Setting Up Embedded Python for OCR
echo ========================================
echo.

REM 创建目录
if not exist "python-embed" mkdir python-embed
cd python-embed

REM 下载 Python 嵌入式版本 (3.11.9)
echo [1/5] Downloading Python Embeddable Package...
echo This may take a few minutes (about 25MB)...
curl -L -o python-3.11.9-embed-amd64.zip https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip

if errorlevel 1 (
    echo [ERROR] Failed to download Python
    echo Please download manually from:
    echo https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip
    pause
    exit /b 1
)

REM 解压
echo.
echo [2/5] Extracting Python...
tar -xf python-3.11.9-embed-amd64.zip
del python-3.11.9-embed-amd64.zip

REM 启用 pip（修改 python311._pth）
echo.
echo [3/5] Enabling pip...
echo python311.zip> python311._pth
echo .>> python311._pth
echo Lib\site-packages>> python311._pth
echo import site>> python311._pth

REM 下载 get-pip.py
echo.
echo [4/5] Installing pip...
curl -L -o get-pip.py https://bootstrap.pypa.io/get-pip.py
.\python.exe get-pip.py
del get-pip.py

REM 安装 OCR 依赖
echo.
echo [5/5] Installing OCR dependencies...
echo This will take 5-10 minutes and download about 500MB...
.\python.exe -m pip install --no-warn-script-location easyocr PyMuPDF

if errorlevel 1 (
    echo [ERROR] Failed to install dependencies
    pause
    exit /b 1
)

cd ..

echo.
echo ========================================
echo [SUCCESS] Embedded Python Setup Complete!
echo ========================================
echo.
echo Python location: python-embed\
echo Size: 
dir python-embed | find "File(s)"
echo.
echo Next steps:
echo 1. Test: python-embed\python.exe src-tauri\scripts\pdf_ocr.py test.pdf
echo 2. Run: build_ocr_with_python.bat
echo.
pause
