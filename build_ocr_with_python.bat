@echo off
echo ========================================
echo Building OCR with Embedded Python
echo ========================================
echo.

REM 检查嵌入式 Python 是否存在
if not exist "python-embed\python.exe" (
    echo [ERROR] Embedded Python not found!
    echo Please run setup_embedded_python.bat first
    pause
    exit /b 1
)

REM 创建 OCR 包目录
echo [1/3] Creating OCR package directory...
if exist "dist\ocr-package" rmdir /s /q dist\ocr-package
mkdir dist\ocr-package

REM 复制 Python 运行时
echo.
echo [2/3] Copying Python runtime...
xcopy /E /I /Y python-embed dist\ocr-package\python

REM 复制 OCR 脚本
echo.
echo [3/3] Copying OCR script...
copy src-tauri\scripts\pdf_ocr.py dist\ocr-package\

REM 创建启动脚本
echo @echo off> dist\ocr-package\pdf_ocr.bat
echo %%~dp0python\python.exe "%%~dp0pdf_ocr.py" %%*>> dist\ocr-package\pdf_ocr.bat

REM 测试
echo.
echo Testing OCR package...
if exist "test.pdf" (
    dist\ocr-package\pdf_ocr.bat test.pdf
    if errorlevel 1 (
        echo [WARNING] Test failed, but package created
    ) else (
        echo [SUCCESS] Test passed!
    )
) else (
    echo [INFO] No test.pdf found, skipping test
)

echo.
echo ========================================
echo [SUCCESS] OCR Package Created!
echo ========================================
echo.
echo Package location: dist\ocr-package\
echo Size:
dir dist\ocr-package | find "File(s)"
echo.
echo Contents:
echo - python\          (Python runtime + dependencies)
echo - pdf_ocr.py       (OCR script)
echo - pdf_ocr.bat      (Launcher)
echo.
echo Next step: Run build_full_with_python.bat
echo.
pause
