# 最终构建解决方案
Write-Host "CheersAI Vault 最终构建解决方案" -ForegroundColor Green
Write-Host "=================================" -ForegroundColor Green

# 检查ICO文件
$icoPath = "src-tauri\icons\icon.ico"
if (Test-Path $icoPath) {
    $header = Get-Content $icoPath -AsByteStream -TotalCount 4
    $headerHex = ($header | ForEach-Object { "{0:X2}" -f $_ }) -join " "
    Write-Host "当前ICO文件头部: $headerHex" -ForegroundColor Cyan
    
    if ($headerHex -eq "89 50 4E 47") {
        Write-Host "✗ 这是PNG文件，不是ICO文件！" -ForegroundColor Red
        Write-Host "请按以下步骤操作：" -ForegroundColor Yellow
        Write-Host "1. 访问 https://convertio.co/jpg-ico/" -ForegroundColor Cyan
        Write-Host "2. 上传您的 public/logo.jpg" -ForegroundColor Cyan
        Write-Host "3. 下载真正的ICO文件" -ForegroundColor Cyan
        Write-Host "4. 将下载的文件重命名为 icon.ico" -ForegroundColor Cyan
        Write-Host "5. 替换 src-tauri\icons\icon.ico" -ForegroundColor Cyan
        Write-Host "6. 重新运行此脚本" -ForegroundColor Cyan
        exit 1
    } elseif ($headerHex -eq "00 00 01 00") {
        Write-Host "✓ 这是正确的ICO文件格式" -ForegroundColor Green
    } else {
        Write-Host "⚠ 未知的文件格式: $headerHex" -ForegroundColor Yellow
    }
} else {
    Write-Host "✗ ICO文件不存在: $icoPath" -ForegroundColor Red
    exit 1
}

# 如果ICO文件正确，开始构建
Write-Host "开始构建应用..." -ForegroundColor Yellow
./build-with-icons.ps1 -BuildType build

if ($LASTEXITCODE -eq 0) {
    Write-Host "✓ 构建成功！" -ForegroundColor Green
    Write-Host "查找生成的安装包..." -ForegroundColor Cyan
    
    $bundlePath = "src-tauri\target\release\bundle"
    if (Test-Path $bundlePath) {
        Get-ChildItem $bundlePath -Recurse -File | Where-Object { 
            $_.Extension -in @(".exe", ".msi") 
        } | ForEach-Object {
            Write-Host "✓ 生成的安装包: $($_.FullName)" -ForegroundColor Green
        }
    }
} else {
    Write-Host "✗ 构建失败" -ForegroundColor Red
}