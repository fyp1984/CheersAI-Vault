# FileBay 连接测试脚本

Write-Host "=== FileBay 服务连接测试 ===" -ForegroundColor Cyan
Write-Host ""

# 读取配置文件
$configPath = "filebay-config.json"
if (Test-Path $configPath) {
    Write-Host "✓ 找到配置文件: $configPath" -ForegroundColor Green
    $config = Get-Content $configPath | ConvertFrom-Json
    
    Write-Host "配置信息:" -ForegroundColor Yellow
    Write-Host "  URL: $($config.url)"
    Write-Host "  用户名: $($config.username)"
    Write-Host "  仓库: $($config.repoName)"
    Write-Host "  邮箱: $($config.email)"
    Write-Host "  Token: $($config.token.Substring(0, 8))..." 
    Write-Host ""
} else {
    Write-Host "✗ 未找到配置文件: $configPath" -ForegroundColor Red
    Write-Host "请先从 Desktop 在线工作区下载配置文件" -ForegroundColor Yellow
    exit 1
}

# 测试 1: 检查 FileBay 服务器是否可访问
Write-Host "测试 1: 检查 FileBay 服务器连接..." -ForegroundColor Cyan
try {
    # 忽略 SSL 证书错误（UAT 环境）
    [System.Net.ServicePointManager]::ServerCertificateValidationCallback = {$true}
    
    $versionUrl = "$($config.url)/api/v1/version"
    Write-Host "  请求: $versionUrl"
    
    $response = Invoke-WebRequest -Uri $versionUrl -Method Get -UseBasicParsing -TimeoutSec 10
    
    if ($response.StatusCode -eq 200) {
        Write-Host "  ✓ 服务器可访问 (HTTP $($response.StatusCode))" -ForegroundColor Green
        Write-Host "  响应: $($response.Content)" -ForegroundColor Gray
    }
} catch {
    Write-Host "  ✗ 服务器连接失败: $($_.Exception.Message)" -ForegroundColor Red
}
Write-Host ""

# 测试 2: 验证 Token 是否有效
Write-Host "测试 2: 验证访问令牌..." -ForegroundColor Cyan
try {
    $userUrl = "$($config.url)/api/v1/user"
    Write-Host "  请求: $userUrl"
    
    $headers = @{
        "Authorization" = "token $($config.token)"
    }
    
    $response = Invoke-WebRequest -Uri $userUrl -Method Get -Headers $headers -UseBasicParsing -TimeoutSec 10
    
    if ($response.StatusCode -eq 200) {
        Write-Host "  ✓ Token 验证成功" -ForegroundColor Green
        $user = $response.Content | ConvertFrom-Json
        Write-Host "  用户信息: $($user.login) ($($user.full_name))" -ForegroundColor Gray
    }
} catch {
    Write-Host "  ✗ Token 验证失败: $($_.Exception.Message)" -ForegroundColor Red
    Write-Host "  请检查 Token 是否正确或已过期" -ForegroundColor Yellow
}
Write-Host ""

# 测试 3: 检查仓库是否存在
Write-Host "测试 3: 检查仓库..." -ForegroundColor Cyan
try {
    $repoUrl = "$($config.url)/api/v1/repos/$($config.username)/$($config.repoName)"
    Write-Host "  请求: $repoUrl"
    
    $headers = @{
        "Authorization" = "token $($config.token)"
    }
    
    $response = Invoke-WebRequest -Uri $repoUrl -Method Get -Headers $headers -UseBasicParsing -TimeoutSec 10
    
    if ($response.StatusCode -eq 200) {
        Write-Host "  ✓ 仓库存在" -ForegroundColor Green
        $repo = $response.Content | ConvertFrom-Json
        Write-Host "  仓库信息: $($repo.full_name)" -ForegroundColor Gray
        Write-Host "  私有: $($repo.private)" -ForegroundColor Gray
        Write-Host "  默认分支: $($repo.default_branch)" -ForegroundColor Gray
    }
} catch {
    if ($_.Exception.Response.StatusCode -eq 404) {
        Write-Host "  ⚠ 仓库不存在，需要创建" -ForegroundColor Yellow
    } else {
        Write-Host "  ✗ 检查仓库失败: $($_.Exception.Message)" -ForegroundColor Red
    }
}
Write-Host ""

# 测试 4: 测试文件上传（创建测试文件）
Write-Host "测试 4: 测试文件上传..." -ForegroundColor Cyan
try {
    $testContent = "这是一个测试文件，用于验证 FileBay 上传功能。`n生成时间: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
    $testContentBase64 = [Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes($testContent))
    
    $uploadUrl = "$($config.url)/api/v1/repos/$($config.username)/$($config.repoName)/contents/test/connection-test.txt"
    Write-Host "  请求: $uploadUrl"
    
    $headers = @{
        "Authorization" = "token $($config.token)"
        "Content-Type" = "application/json"
    }
    
    $body = @{
        message = "测试连接 - $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
        content = $testContentBase64
        branch = "main"
    } | ConvertTo-Json
    
    $response = Invoke-WebRequest -Uri $uploadUrl -Method Post -Headers $headers -Body $body -UseBasicParsing -TimeoutSec 10
    
    if ($response.StatusCode -eq 201) {
        Write-Host "  ✓ 文件上传成功" -ForegroundColor Green
        $result = $response.Content | ConvertFrom-Json
        Write-Host "  文件路径: $($result.content.path)" -ForegroundColor Gray
        Write-Host "  提交 SHA: $($result.commit.sha.Substring(0, 8))..." -ForegroundColor Gray
    }
} catch {
    if ($_.Exception.Response.StatusCode -eq 422) {
        Write-Host "  ⚠ 文件已存在（这是正常的）" -ForegroundColor Yellow
    } else {
        Write-Host "  ✗ 文件上传失败: $($_.Exception.Message)" -ForegroundColor Red
    }
}
Write-Host ""

Write-Host "=== 测试完成 ===" -ForegroundColor Cyan
