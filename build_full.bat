@echo off
echo ========================================
echo Building Full Version with OCR
echo ========================================
echo.

REM Step 1: Build OCR executable
echo [1/3] Building OCR executable...
call build_ocr.bat
if errorlevel 1 (
    echo [ERROR] OCR build failed
    pause
    exit /b 1
)

REM Step 2: Update Tauri config to include OCR
echo.
echo [2/3] Updating Tauri configuration...
powershell -Command "(Get-Content src-tauri\tauri.conf.json) -replace '\"scripts/\*\"', '\"scripts/*\",\n      \"../dist/pdf_ocr.exe\"' | Set-Content src-tauri\tauri.conf.json"

REM Step 3: Build Tauri application
echo.
echo [3/3] Building Tauri application...
pnpm tauri build

if errorlevel 1 (
    echo [ERROR] Tauri build failed
    REM Restore original config
    powershell -Command "(Get-Content src-tauri\tauri.conf.json) -replace '\"scripts/\*\",\s*\"../dist/pdf_ocr.exe\"', '\"scripts/*\"' | Set-Content src-tauri\tauri.conf.json"
    pause
    exit /b 1
)

REM Restore original config
echo.
echo Restoring configuration...
powershell -Command "(Get-Content src-tauri\tauri.conf.json) -replace '\"scripts/\*\",\s*\"../dist/pdf_ocr.exe\"', '\"scripts/*\"' | Set-Content src-tauri\tauri.conf.json"

echo.
echo ========================================
echo [SUCCESS] Full Version Built!
echo ========================================
echo.
echo Installer location:
dir src-tauri\target\release\bundle\nsis\*.exe
echo.
echo This version includes OCR functionality.
echo.
pause
