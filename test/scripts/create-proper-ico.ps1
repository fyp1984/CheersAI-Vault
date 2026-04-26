# 创建正确格式的ICO文件
# 使用ImageMagick或在线工具生成

param(
    [string]$LogoPath = "public/logo.jpg",
    [string]$OutputPath = "src-tauri/icons/icon.ico"
)

Write-Host "创建正确格式的ICO文件..." -ForegroundColor Green

# 检查是否有ImageMagick
try {
    magick -version | Out-Null
    Write-Host "使用ImageMagick生成ICO文件..." -ForegroundColor Cyan
    
    # 使用ImageMagick创建多尺寸ICO文件
    magick $LogoPath -resize 256x256 -define icon:auto-resize=256,128,64,48,32,16 $OutputPath
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ ICO文件生成成功: $OutputPath" -ForegroundColor Green
    } else {
        throw "ImageMagick生成失败"
    }
} catch {
    Write-Host "ImageMagick不可用，使用备用方案..." -ForegroundColor Yellow
    
    # 备用方案：删除现有的ICO文件，让Tauri使用PNG
    if (Test-Path $OutputPath) {
        Remove-Item $OutputPath -Force
        Write-Host "已删除无效的ICO文件" -ForegroundColor Yellow
    }
    
    Write-Host "建议安装ImageMagick或使用在线工具生成ICO文件" -ForegroundColor Yellow
    Write-Host "在线工具: https://convertio.co/jpg-ico/" -ForegroundColor Cyan
}