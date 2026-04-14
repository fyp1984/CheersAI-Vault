@echo off
echo ========================================
echo Building Full Version with Python OCR
echo ========================================
echo.

REM 检查 OCR 包是否存在
if not exist "dist\ocr-package\python\python.exe" (
    echo [ERROR] OCR package not found!
    echo Please run build_ocr_with_python.bat first
    pause
    exit /b 1
)

REM 更新 Tauri 配置
echo [1/2] Updating Tauri configuration...
powershell -Command "(Get-Content src-tauri\tauri.conf.json) -replace '\"scripts/\*\"', '\"scripts/*\",\n      \"../dist/ocr-package\"' | Set-Content src-tauri\tauri.conf.json"

REM 构建应用
echo.
echo [2/2] Building Tauri application...
echo This will take several minutes...
pnpm tauri build

if errorlevel 1 (
    echo [ERROR] Build failed!
    REM 恢复配置
    powershell -Command "(Get-Content src-tauri\tauri.conf.json) -replace '\"scripts/\*\",\s*\"../dist/ocr-package\"', '\"scripts/*\"' | Set-Content src-tauri\tauri.conf.json"
    pause
    exit /b 1
)

REM 恢复配置
echo.
echo Restoring configuration...
powershell -Command "(Get-Content src-tauri\tauri.conf.json) -replace '\"scripts/\*\",\s*\"../dist/ocr-package\"', '\"scripts/*\"' | Set-Content src-tauri\tauri.conf.json"

echo.
echo ========================================
echo [SUCCESS] Full Version Built!
echo ========================================
echo.
echo Installer location:
dir src-tauri\target\release\bundle\nsis\*.exe 2>nul
dir src-tauri\target\release\bundle\msi\*.msi 2>nul
echo.
echo This version includes:
echo - Python runtime (embedded)
echo - OCR dependencies (easyocr, PyMuPDF)
echo - All application features
echo.
echo Users can use OCR without installing Python!
echo.
pause
