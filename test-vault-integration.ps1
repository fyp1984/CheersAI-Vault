# Vault 集成测试脚本

Write-Host "=== Vault 集成测试 ===" -ForegroundColor Cyan
Write-Host ""

# 1. 检查 Vault 数据库是否存在
$vaultDbPath = "$env:USERPROFILE\.cheersai\vault.db"
Write-Host "1. 检查 Vault 数据库..." -ForegroundColor Yellow
Write-Host "   路径: $vaultDbPath"

if (Test-Path $vaultDbPath) {
    Write-Host "   ✅ 数据库文件存在" -ForegroundColor Green
    
    # 获取文件大小
    $fileSize = (Get-Item $vaultDbPath).Length
    Write-Host "   文件大小: $fileSize 字节"
    
    # 查询数据库内容
    Write-Host ""
    Write-Host "2. 查询数据库内容..." -ForegroundColor Yellow
    
    try {
        # 使用 SQLite 查询（需要安装 sqlite3）
        $query = "SELECT user_id, email, username, repo_name, updated_at FROM filebay_configs;"
        $result = sqlite3 $vaultDbPath $query 2>&1
        
        if ($LASTEXITCODE -eq 0) {
            if ($result) {
                Write-Host "   ✅ 找到配置:" -ForegroundColor Green
                Write-Host "   $result"
            } else {
                Write-Host "   ⚠️  数据库为空，没有配置" -ForegroundColor Yellow
                Write-Host "   请访问 http://localhost:3000/sync-config 同步配置"
            }
        } else {
            Write-Host "   ⚠️  无法查询数据库（可能需要安装 sqlite3）" -ForegroundColor Yellow
            Write-Host "   错误: $result"
        }
    } catch {
        Write-Host "   ⚠️  查询失败: $_" -ForegroundColor Yellow
    }
} else {
    Write-Host "   ❌ 数据库文件不存在" -ForegroundColor Red
    Write-Host "   请先在 Vault 系统中登录并同步配置"
    Write-Host "   访问: http://localhost:3000/sync-config"
}

Write-Host ""
Write-Host "3. 检查 Vault Bridge 服务..." -ForegroundColor Yellow

try {
    $response = Invoke-WebRequest -Uri "http://localhost:8765/health" -TimeoutSec 2 -ErrorAction Stop
    $health = $response.Content | ConvertFrom-Json
    
    Write-Host "   ✅ Vault Bridge 服务正在运行" -ForegroundColor Green
    Write-Host "   版本: $($health.version)"
    Write-Host "   状态: $($health.status)"
    Write-Host "   数据库: $($health.database)"
} catch {
    Write-Host "   ❌ Vault Bridge 服务未运行" -ForegroundColor Red
    Write-Host "   请在 CheersAI-Desktop 项目中启动服务:"
    Write-Host "   cd E:\CheersAI-Desktop"
    Write-Host "   .\start_vault_bridge.ps1"
}

Write-Host ""
Write-Host "4. 检查脱敏程序编译状态..." -ForegroundColor Yellow

$cargoTomlPath = "src-tauri\Cargo.toml"
if (Test-Path $cargoTomlPath) {
    Write-Host "   ✅ 找到 Cargo.toml" -ForegroundColor Green
    
    # 检查 vault 模块是否存在
    $vaultModulePath = "src-tauri\src\commands\vault.rs"
    if (Test-Path $vaultModulePath) {
        Write-Host "   ✅ Vault 模块已创建" -ForegroundColor Green
        
        # 获取文件行数
        $lineCount = (Get-Content $vaultModulePath).Count
        Write-Host "   模块大小: $lineCount 行"
    } else {
        Write-Host "   ❌ Vault 模块不存在" -ForegroundColor Red
    }
} else {
    Write-Host "   ❌ 未找到 Cargo.toml" -ForegroundColor Red
}

Write-Host ""
Write-Host "=== 测试完成 ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "📋 下一步操作:" -ForegroundColor Yellow
Write-Host "1. 如果 Vault Bridge 未运行，请启动它"
Write-Host "2. 如果数据库为空，请访问 http://localhost:3000/sync-config 同步配置"
Write-Host "3. 启动脱敏程序: pnpm tauri dev"
Write-Host "4. 进入 FileBay 设置页面，点击'从 Vault 加载'按钮"
Write-Host ""
