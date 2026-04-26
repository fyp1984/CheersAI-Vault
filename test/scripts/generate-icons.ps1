# CheersAI Vault 图标生成脚本
# 需要安装 ImageMagick: https://imagemagick.org/script/download.php#windows

param(
    [string]$LogoPath = "public/logo.jpg",
    [string]$OutputDir = "src-tauri/icons"
)

Write-Host "CheersAI Vault 图标生成工具" -ForegroundColor Green
Write-Host "================================" -ForegroundColor Green

# 检查 ImageMagick 是否安装
try {
    magick -version | Out-Null
    Write-Host "✓ ImageMagick 已安装" -ForegroundColor Green
} catch {
    Write-Host "✗ 未找到 ImageMagick，请先安装：https://imagemagick.org/script/download.php#windows" -ForegroundColor Red
    exit 1
}

# 检查源文件是否存在
if (-not (Test-Path $LogoPath)) {
    Write-Host "✗ 未找到源文件：$LogoPath" -ForegroundColor Red
    exit 1
}

Write-Host "✓ 源文件：$LogoPath" -ForegroundColor Green

# 确保输出目录存在
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

Write-Host "正在生成图标..." -ForegroundColor Yellow

# 生成基本图标
$icons = @(
    @{Size="32x32"; File="32x32.png"},
    @{Size="128x128"; File="128x128.png"},
    @{Size="256x256"; File="128x128@2x.png"},
    @{Size="512x512"; File="icon.png"}
)

foreach ($icon in $icons) {
    $outputPath = Join-Path $OutputDir $icon.File
    Write-Host "  生成 $($icon.File) ($($icon.Size))" -ForegroundColor Cyan
    magick $LogoPath -resize $icon.Size $outputPath
}

# 生成 ICO 文件
Write-Host "  生成 icon.ico" -ForegroundColor Cyan
magick $LogoPath -resize 256x256 (Join-Path $OutputDir "icon.ico")

# 生成 Windows Store 图标
$storeIcons = @(
    @{Size="30x30"; File="Square30x30Logo.png"},
    @{Size="44x44"; File="Square44x44Logo.png"},
    @{Size="71x71"; File="Square71x71Logo.png"},
    @{Size="89x89"; File="Square89x89Logo.png"},
    @{Size="107x107"; File="Square107x107Logo.png"},
    @{Size="142x142"; File="Square142x142Logo.png"},
    @{Size="150x150"; File="Square150x150Logo.png"},
    @{Size="284x284"; File="Square284x284Logo.png"},
    @{Size="310x310"; File="Square310x310Logo.png"},
    @{Size="50x50"; File="StoreLogo.png"}
)

Write-Host "  生成 Windows Store 图标..." -ForegroundColor Cyan
foreach ($icon in $storeIcons) {
    $outputPath = Join-Path $OutputDir $icon.File
    magick $LogoPath -resize $icon.Size $outputPath
}

# 生成 macOS ICNS 文件（简化版本）
Write-Host "  生成 icon.icns" -ForegroundColor Cyan
$iconsetDir = Join-Path $OutputDir "icon.iconset"
if (Test-Path $iconsetDir) {
    Remove-Item $iconsetDir -Recurse -Force
}
New-Item -ItemType Directory -Path $iconsetDir -Force | Out-Null

$icnsIcons = @(
    @{Size="16x16"; File="icon_16x16.png"},
    @{Size="32x32"; File="icon_16x16@2x.png"},
    @{Size="32x32"; File="icon_32x32.png"},
    @{Size="64x64"; File="icon_32x32@2x.png"},
    @{Size="128x128"; File="icon_128x128.png"},
    @{Size="256x256"; File="icon_128x128@2x.png"},
    @{Size="256x256"; File="icon_256x256.png"},
    @{Size="512x512"; File="icon_256x256@2x.png"},
    @{Size="512x512"; File="icon_512x512.png"},
    @{Size="1024x1024"; File="icon_512x512@2x.png"}
)

foreach ($icon in $icnsIcons) {
    $outputPath = Join-Path $iconsetDir $icon.File
    magick $LogoPath -resize $icon.Size $outputPath
}

# 尝试生成 ICNS 文件（需要 macOS 或额外工具）
try {
    if (Get-Command iconutil -ErrorAction SilentlyContinue) {
        iconutil -c icns $iconsetDir
        Write-Host "  ✓ 成功生成 icon.icns" -ForegroundColor Green
    } else {
        Write-Host "  ⚠ 无法生成 icon.icns (需要 macOS 或 iconutil 工具)" -ForegroundColor Yellow
        Write-Host "    请使用在线工具转换：https://convertio.co/png-icns/" -ForegroundColor Yellow
    }
} catch {
    Write-Host "  ⚠ 生成 icon.icns 失败，请手动转换" -ForegroundColor Yellow
}

# 清理临时文件
Remove-Item $iconsetDir -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "" 
Write-Host "✓ 图标生成完成！" -ForegroundColor Green
Write-Host "现在可以运行以下命令重新构建应用：" -ForegroundColor Cyan
Write-Host "  pnpm tauri build" -ForegroundColor White
Write-Host ""