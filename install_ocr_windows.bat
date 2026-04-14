@echo off
echo ========================================
echo CheersAI Vault - OCR Setup
echo ========================================
echo.

REM Check if Python is installed
python --version >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Python is not installed or not in PATH
    echo.
    echo Please install Python 3.7+ from https://www.python.org/
    echo Make sure to check "Add Python to PATH" during installation
    echo.
    pause
    exit /b 1
)

echo [OK] Python found!
python --version
echo.

REM Install required packages
echo Installing OCR dependencies...
echo This may take several minutes...
echo.

pip install --upgrade pip
pip install easyocr PyMuPDF

if errorlevel 1 (
    echo.
    echo [ERROR] Failed to install dependencies
    echo.
    echo Please try manually:
    echo   pip install easyocr PyMuPDF
    echo.
    pause
    exit /b 1
)

echo.
echo ========================================
echo [SUCCESS] OCR Setup Complete!
echo ========================================
echo.
echo The application will now be able to process scanned PDFs.
echo Note: First OCR use will download AI models (~100MB)
echo.
pause
