# 简化的图标转换脚本
# 使用Windows内置的.NET功能来调整图像大小

param(
    [string]$LogoPath = "public/logo.jpg",
    [string]$OutputDir = "src-tauri/icons"
)

Write-Host "CheersAI Vault 图标转换工具" -ForegroundColor Green
Write-Host "================================" -ForegroundColor Green

# 检查源文件
if (-not (Test-Path $LogoPath)) {
    Write-Host "✗ 未找到源文件：$LogoPath" -ForegroundColor Red
    exit 1
}

# 确保输出目录存在
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
    Write-Host "✓ 创建输出目录：$OutputDir" -ForegroundColor Green
}

Write-Host "✓ 源文件：$LogoPath" -ForegroundColor Green

# 加载.NET图像处理类
Add-Type -AssemblyName System.Drawing

try {
    # 读取原始图像 - 使用绝对路径
    $logoFullPath = (Resolve-Path $LogoPath).Path
    $originalImage = [System.Drawing.Image]::FromFile($logoFullPath)
    Write-Host "✓ 原始图像尺寸：$($originalImage.Width)x$($originalImage.Height)" -ForegroundColor Green
    
    # 定义需要的尺寸
    $sizes = @(
        @{Size=32; File="32x32.png"},
        @{Size=128; File="128x128.png"},
        @{Size=256; File="128x128@2x.png"},
        @{Size=512; File="icon.png"}
    )
    
    foreach ($sizeInfo in $sizes) {
        $size = $sizeInfo.Size
        $filename = $sizeInfo.File
        $outputPath = Join-Path (Resolve-Path $OutputDir).Path $filename
        
        Write-Host "  生成 $filename ($($size)x$($size))" -ForegroundColor Cyan
        
        # 创建新的位图
        $newImage = New-Object System.Drawing.Bitmap($size, $size)
        $graphics = [System.Drawing.Graphics]::FromImage($newImage)
        
        # 设置高质量缩放
        $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
        $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        
        # 绘制缩放后的图像
        $graphics.DrawImage($originalImage, 0, 0, $size, $size)
        
        # 保存为PNG
        $newImage.Save($outputPath, [System.Drawing.Imaging.ImageFormat]::Png)
        
        # 清理资源
        $graphics.Dispose()
        $newImage.Dispose()
    }
    
    # 生成 ICO 文件 (Windows)
    Write-Host "  生成 icon.ico" -ForegroundColor Cyan
    $icoPath = Join-Path (Resolve-Path $OutputDir).Path "icon.ico"
    
    # 创建256x256的ICO文件（简化版本）
    $ico256 = New-Object System.Drawing.Bitmap(256, 256)
    $g256 = [System.Drawing.Graphics]::FromImage($ico256)
    $g256.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g256.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g256.DrawImage($originalImage, 0, 0, 256, 256)
    
    # 保存为PNG格式（Windows会识别为图标）
    $ico256.Save($icoPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $g256.Dispose()
    $ico256.Dispose()
    
    # 生成Windows Store图标
    Write-Host "  生成 Windows Store 图标" -ForegroundColor Cyan
    $storeIcons = @(
        @{Size=30; File="Square30x30Logo.png"},
        @{Size=44; File="Square44x44Logo.png"},
        @{Size=71; File="Square71x71Logo.png"},
        @{Size=89; File="Square89x89Logo.png"},
        @{Size=107; File="Square107x107Logo.png"},
        @{Size=142; File="Square142x142Logo.png"},
        @{Size=150; File="Square150x150Logo.png"},
        @{Size=284; File="Square284x284Logo.png"},
        @{Size=310; File="Square310x310Logo.png"},
        @{Size=50; File="StoreLogo.png"}
    )
    
    foreach ($storeIcon in $storeIcons) {
        $size = $storeIcon.Size
        $filename = $storeIcon.File
        $outputPath = Join-Path (Resolve-Path $OutputDir).Path $filename
        
        $storeImage = New-Object System.Drawing.Bitmap($size, $size)
        $storeGraphics = [System.Drawing.Graphics]::FromImage($storeImage)
        $storeGraphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $storeGraphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
        $storeGraphics.DrawImage($originalImage, 0, 0, $size, $size)
        $storeImage.Save($outputPath, [System.Drawing.Imaging.ImageFormat]::Png)
        $storeGraphics.Dispose()
        $storeImage.Dispose()
    }
    
    Write-Host "  ⚠ icon.icns 需要手动转换或使用在线工具" -ForegroundColor Yellow
    
    # 清理原始图像
    $originalImage.Dispose()
    
    Write-Host ""
    Write-Host "✓ 图标生成完成！" -ForegroundColor Green
    Write-Host "现在可以重新构建应用：pnpm tauri build" -ForegroundColor Cyan
    
} catch {
    Write-Host "✗ 图标生成失败：$($_.Exception.Message)" -ForegroundColor Red
    exit 1
}