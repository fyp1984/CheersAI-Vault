@echo off
echo ========================================
echo Building Application with OCR
echo ========================================
echo.

REM 检查 OCR 包是否存在
if not exist "python-embed\python.exe" (
    echo [ERROR] Python embed not found!
    echo Please run: setup_embedded_python.bat
    pause
    exit /b 1
)

REM 步骤 1: 构建前端
echo [1/5] Building frontend...
call pnpm build
if errorlevel 1 (
    echo [ERROR] Frontend build failed
    pause
    exit /b 1
)

REM 步骤 2: 将 OCR 包复制到 src-tauri/resources
echo.
echo [2/5] Copying OCR package to resources...
if not exist "src-tauri\resources" mkdir src-tauri\resources
if exist "src-tauri\resources\ocr-package" rmdir /s /q src-tauri\resources\ocr-package
mkdir src-tauri\resources\ocr-package
xcopy /E /I /Y python-embed src-tauri\resources\ocr-package\python >nul
copy src-tauri\scripts\pdf_ocr.py src-tauri\resources\ocr-package\ >nul

REM 步骤 3: 更新 Tauri 配置
echo.
echo [3/5] Updating Tauri configuration...
powershell -Command "$config = Get-Content 'src-tauri\tauri.conf.json' -Raw | ConvertFrom-Json; $config.bundle.resources = @('scripts/*', 'resources/ocr-package'); $config | ConvertTo-Json -Depth 10 | Set-Content 'src-tauri\tauri.conf.json'"

REM 步骤 4: 构建 Tauri 应用
echo.
echo [4/5] Building Tauri application...
echo This will take 5-10 minutes...
powershell -Command "$env:PATH = \"$env:USERPROFILE\.cargo\bin;$env:PATH\"; pnpm tauri build"

if errorlevel 1 (
    echo [ERROR] Tauri build failed
    REM 恢复配置
    powershell -Command "$config = Get-Content 'src-tauri\tauri.conf.json' -Raw | ConvertFrom-Json; $config.bundle.resources = @('scripts/*'); $config | ConvertTo-Json -Depth 10 | Set-Content 'src-tauri\tauri.conf.json'"
    pause
    exit /b 1
)

REM 步骤 5: 恢复配置
echo.
echo [5/5] Restoring configuration...
powershell -Command "$config = Get-Content 'src-tauri\tauri.conf.json' -Raw | ConvertFrom-Json; $config.bundle.resources = @('scripts/*'); $config | ConvertTo-Json -Depth 10 | Set-Content 'src-tauri\tauri.conf.json'"

echo.
echo ========================================
echo [SUCCESS] Build Complete!
echo ========================================
echo.
echo Installer location:
dir src-tauri\target\release\bundle\nsis\*.exe 2>nul
echo.
echo This version includes embedded Python OCR (~600MB)
echo.
pause
