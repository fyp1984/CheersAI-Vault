@echo off
echo ========================================
echo Compressing OCR Package
echo ========================================
echo.

REM 检查 Python 环境
if not exist "python-embed\python.exe" (
    echo [ERROR] Python embed not found!
    echo Please run: setup_embedded_python.bat
    pause
    exit /b 1
)

REM 创建压缩包
echo Compressing Python environment...
echo This may take 5-10 minutes...

powershell -Command "Compress-Archive -Path 'python-embed\*' -DestinationPath 'src-tauri\resources\python-ocr.zip' -Force -CompressionLevel Optimal"

if errorlevel 1 (
    echo [ERROR] Compression failed
    pause
    exit /b 1
)

REM 复制 OCR 脚本
if not exist "src-tauri\resources" mkdir src-tauri\resources
copy src-tauri\scripts\pdf_ocr.py src-tauri\resources\ >nul

echo.
echo ========================================
echo [SUCCESS] OCR Package Compressed!
echo ========================================
echo.
echo Compressed file: src-tauri\resources\python-ocr.zip
dir src-tauri\resources\python-ocr.zip | find "python-ocr.zip"
echo.
echo This single file will be included in the installer.
echo On first run, it will be extracted automatically.
echo.
pause
