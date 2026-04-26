# CheersAI Vault 图标编译脚本
# 解决中文路径导致的Windows资源编译器问题

param(
    [string]$BuildType = "build"  # "build" 或 "dev"
)

Write-Host "CheersAI Vault 图标编译工具" -ForegroundColor Green
Write-Host "================================" -ForegroundColor Green

# 获取当前路径
$currentPath = Get-Location
Write-Host "当前路径: $currentPath" -ForegroundColor Cyan

# 创建临时英文路径
$tempPath = "C:\temp\cheersai-vault-build"
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"

Write-Host "创建临时构建目录..." -ForegroundColor Yellow

# 清理旧的临时目录
if (Test-Path $tempPath) {
    Remove-Item $tempPath -Recurse -Force -ErrorAction SilentlyContinue
}

# 创建新的临时目录
New-Item -ItemType Directory -Path $tempPath -Force | Out-Null

try {
    Write-Host "复制项目文件到临时目录..." -ForegroundColor Yellow
    
    # 复制所有必要文件，排除 node_modules 和 target
    $excludeItems = @("node_modules", "src-tauri\target", "dist", ".git")
    
    Get-ChildItem -Path $currentPath | Where-Object { 
        $_.Name -notin $excludeItems 
    } | ForEach-Object {
        $dest = Join-Path $tempPath $_.Name
        if ($_.PSIsContainer) {
            Copy-Item $_.FullName $dest -Recurse -Force
        } else {
            Copy-Item $_.FullName $dest -Force
        }
        Write-Host "  复制: $($_.Name)" -ForegroundColor Gray
    }
    
    # 进入临时目录
    Set-Location $tempPath
    Write-Host "切换到临时目录: $tempPath" -ForegroundColor Cyan
    
    # 安装依赖
    Write-Host "安装依赖..." -ForegroundColor Yellow
    pnpm install --frozen-lockfile
    
    if ($LASTEXITCODE -ne 0) {
        throw "依赖安装失败"
    }
    
    # 执行构建
    if ($BuildType -eq "build") {
        Write-Host "开始构建应用..." -ForegroundColor Yellow
        pnpm tauri build
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "构建成功！" -ForegroundColor Green
            
            # 复制构建结果回原目录
            $sourceBundle = Join-Path $tempPath "src-tauri\target\release\bundle"
            $destBundle = Join-Path $currentPath "src-tauri\target\release\bundle"
            
            if (Test-Path $sourceBundle) {
                Write-Host "复制构建结果..." -ForegroundColor Yellow
                
                # 确保目标目录存在
                $destDir = Split-Path $destBundle -Parent
                if (-not (Test-Path $destDir)) {
                    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
                }
                
                Copy-Item $sourceBundle $destDir -Recurse -Force
                Write-Host "构建文件已复制到: $destBundle" -ForegroundColor Green
                
                # 显示生成的文件
                Write-Host "`n生成的安装包:" -ForegroundColor Cyan
                Get-ChildItem $destBundle -Recurse -File | Where-Object { 
                    $_.Extension -in @(".exe", ".msi") 
                } | ForEach-Object {
                    Write-Host "  $($_.FullName)" -ForegroundColor White
                }
            }
        } else {
            throw "构建失败"
        }
    } else {
        Write-Host "启动开发服务器..." -ForegroundColor Yellow
        pnpm tauri dev
    }
    
} catch {
    Write-Host "错误: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
} finally {
    # 返回原目录
    Set-Location $currentPath
    
    # 清理临时目录
    Write-Host "清理临时文件..." -ForegroundColor Yellow
    if (Test-Path $tempPath) {
        Remove-Item $tempPath -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Write-Host "`n构建完成！" -ForegroundColor Green