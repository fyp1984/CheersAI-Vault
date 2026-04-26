# 修复ICO文件并构建应用
param(
    [string]$LogoPath = "public/logo.jpg"
)

Write-Host "修复ICO文件并构建CheersAI Vault..." -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green

# 检查ImageMagick
$hasImageMagick = $false
try {
    magick -version | Out-Null
    $hasImageMagick = $true
    Write-Host "✓ 检测到ImageMagick" -ForegroundColor Green
} catch {
    Write-Host "⚠ 未检测到ImageMagick" -ForegroundColor Yellow
}

if ($hasImageMagick) {
    Write-Host "使用ImageMagick生成ICO文件..." -ForegroundColor Cyan
    
    # 生成正确格式的ICO文件
    magick $LogoPath -resize 256x256 -define icon:auto-resize=256,128,64,48,32,16 "src-tauri/icons/icon.ico"
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ ICO文件生成成功" -ForegroundColor Green
    } else {
        Write-Host "✗ ICO文件生成失败" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "下载并安装ImageMagick..." -ForegroundColor Yellow
    
    # 尝试使用winget安装ImageMagick
    try {
        winget install --id ImageMagick.ImageMagick.Q16 --silent --accept-package-agreements --accept-source-agreements
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "✓ ImageMagick安装成功，请重新运行脚本" -ForegroundColor Green
            Write-Host "或者手动访问 https://convertio.co/jpg-ico/ 转换logo.jpg为icon.ico" -ForegroundColor Cyan
            exit 0
        } else {
            throw "winget安装失败"
        }
    } catch {
        Write-Host "自动安装失败，请手动处理:" -ForegroundColor Red
        Write-Host "1. 访问 https://convertio.co/jpg-ico/" -ForegroundColor Cyan
        Write-Host "2. 上传 public/logo.jpg" -ForegroundColor Cyan
        Write-Host "3. 下载生成的icon.ico" -ForegroundColor Cyan
        Write-Host "4. 将icon.ico放到 src-tauri/icons/ 目录" -ForegroundColor Cyan
        Write-Host "5. 重新运行构建脚本" -ForegroundColor Cyan
        exit 1
    }
}

# 验证ICO文件
if (Test-Path "src-tauri/icons/icon.ico") {
    $icoSize = (Get-Item "src-tauri/icons/icon.ico").Length
    Write-Host "✓ ICO文件存在，大小: $icoSize 字节" -ForegroundColor Green
    
    # 运行构建
    Write-Host "开始构建应用..." -ForegroundColor Yellow
    ./build-with-icons.ps1 -BuildType build
} else {
    Write-Host "✗ ICO文件不存在，请手动创建" -ForegroundColor Red
    exit 1
}