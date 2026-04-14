@echo off
echo ========================================
echo Building OCR Executable for Distribution
echo ========================================
echo.

REM Check if Python is installed
python --version >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Python is not installed or not in PATH
    echo Please install Python 3.7+ from https://www.python.org/
    pause
    exit /b 1
)

echo [OK] Python found!
python --version
echo.

REM Check if PyInstaller is installed
pip show pyinstaller >nul 2>&1
if errorlevel 1 (
    echo Installing PyInstaller...
    pip install pyinstaller
    if errorlevel 1 (
        echo [ERROR] Failed to install PyInstaller
        pause
        exit /b 1
    )
)

REM Check if dependencies are installed
echo Checking OCR dependencies...
pip show easyocr >nul 2>&1
if errorlevel 1 (
    echo Installing easyocr...
    pip install easyocr
)

pip show PyMuPDF >nul 2>&1
if errorlevel 1 (
    echo Installing PyMuPDF...
    pip install PyMuPDF
)

echo.
echo Building standalone executable...
echo This will take 5-10 minutes and create a ~500MB file
echo.

REM Clean previous builds
if exist build rmdir /s /q build
if exist dist\pdf_ocr.exe del /q dist\pdf_ocr.exe

REM Build the executable
pyinstaller --onefile ^
    --name pdf_ocr ^
    --console ^
    --clean ^
    --noconfirm ^
    src-tauri/scripts/pdf_ocr.py

if errorlevel 1 (
    echo.
    echo [ERROR] Build failed!
    echo.
    pause
    exit /b 1
)

echo.
echo ========================================
echo [SUCCESS] Build Complete!
echo ========================================
echo.
echo Executable: dist\pdf_ocr.exe
dir dist\pdf_ocr.exe | find "pdf_ocr.exe"
echo.
echo Next steps for bundling with application:
echo 1. Test: dist\pdf_ocr.exe test.pdf
echo 2. The exe is already configured in tauri.conf.json
echo 3. Run: pnpm tauri build
echo.
echo The OCR executable will be included in the installer.
echo.
pause

