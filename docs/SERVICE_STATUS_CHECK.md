# 服务状态检查报告

生成时间: 2026-05-06

## ✅ 已启动的服务

### 1. 项目 1：cheersai-desktop（Tauri 脱敏桌面应用）
- **位置**: `e:\CheersAI脱敏\cheersai-desktop`
- **状态**: ✅ 运行中
- **访问地址**: http://localhost:1420/
- **进程**: Terminal 2
- **功能**:
  - 文件脱敏/反脱敏
  - 敏感词管理
  - 沙箱管理
  - FileBay 集成
  - OCR 功能
  - AI 检测（Ollama + qwen2.5:1.5b）

#### Vault API 服务器
- **端口**: 7788
- **状态**: ✅ 运行中
- **健康检查**: http://localhost:7788/api/v1/health
- **FileBay 配置**: 已保存
  - 用户: admin_cheersai_cloud_de8df0
  - 邮箱: admin@cheersai.cloud
  - 服务器: https://uat-filebay.cheersai.cloud

### 2. 项目 2：CheersAI-Desktop（Dify 后端 API）
- **位置**: `e:\CheersAI-Desktop`
- **状态**: ✅ 全部运行中

#### 2.1 Docker 中间件服务
- **状态**: ✅ 运行中（7个容器）
- **服务列表**:
  - PostgreSQL (端口 5432) - 数据库
  - Redis (端口 5700) - 缓存和消息队列
  - Weaviate (端口 8080) - 向量数据库
  - Sandbox - 代码执行沙箱
  - SSRF Proxy (端口 3128, 8194) - 安全代理
  - Unstructured API (端口 8000) - 文档解析
  - Plugin Daemon (端口 5002-5003) - 插件服务

#### 2.2 Flask API 服务
- **端口**: 5001
- **状态**: ✅ 运行中（使用 uv 本地运行）
- **访问地址**: http://localhost:5001
- **调试模式**: 已启用
- **Debugger PIN**: 722-330-360
- **进程**: Terminal 12
- **数据库迁移**: ✅ 已更新到最新版本 (a1b2c3d4e5f7)

#### 2.3 Celery Worker
- **状态**: ✅ 运行中（Docker 容器）
- **容器**: docker-worker-1
- **队列**: 
  - dataset, priority_dataset - 文档索引
  - pipeline, priority_pipeline - RAG Pipeline
  - generation - 生成任务
  - mail - 邮件发送
  - ops_trace - 操作追踪
  - app_deletion - 应用删除

#### 2.4 Celery Beat
- **状态**: ✅ 运行中（Docker 容器）
- **容器**: docker-worker_beat-1
- **功能**: 定时任务调度（文档处理规则自动修复）

#### 2.5 Next.js 前端
- **端口**: 3000
- **状态**: ✅ 运行中
- **访问地址**: http://localhost:3000
- **调试端口**: 9229
- **进程**: Terminal 8
- **模式**: Turbopack

### 3. SSO 服务（Casdoor）
- **状态**: ✅ 运行中（22小时）
- **容器**: 
  - cheersai-sso-casdoor-1 (端口 18000)
  - cheersai-sso-db-1 (端口 13306)

## 📋 FileBay 服务检查

### FileBay 配置
- **服务器**: https://uat-filebay.cheersai.cloud
- **用户名**: admin_cheersai_cloud_de8df0
- **仓库**: workspace
- **邮箱**: admin@cheersai.cloud
- **Token**: 已配置（7cb8cbe2...）
- **配置文件**: filebay-config.json

### 连接测试
FileBay 是外部服务（UAT 环境），需要通过以下方式测试连接：

#### 方式 1：通过脱敏程序界面测试（推荐）
1. 打开脱敏程序（http://localhost:1420/）
2. 点击左侧菜单 "FileBay 设置"
3. 点击 "读取已下载配置" 按钮
4. 点击 "测试连接" 按钮

**原理**: 脱敏程序使用 `fetch_via_browser` 方法，通过隐藏的浏览器窗口发送请求，可以绕过 CORS 和 SSL 证书问题。

#### 方式 2：通过 PowerShell 测试
```powershell
cd e:\CheersAI脱敏\cheersai-desktop
.\test-filebay-connection.ps1
```

**注意**: PowerShell 直接测试可能会遇到 SSL 证书问题（UAT 环境使用自签名证书）。

#### 方式 3：通过 Python 测试
```bash
cd e:\CheersAI-Desktop
python check_services.py
```

## 🔧 已完成的配置

1. ✅ 安装 `uv` (版本 0.11.9)
   - 路径: `C:\Users\33814\.local\bin`
   - 已添加到当前会话 PATH

2. ✅ 安装 Python 依赖
   - 使用 `uv sync --dev` 安装了 490 个包
   - 包括 flask-restful（手动补充安装）

3. ✅ 停止 Docker Flask API 容器
   - 改用本地 uv 运行，便于调试

4. ✅ 启动所有必需服务
   - Tauri 桌面应用
   - Vault API 服务器
   - Flask API
   - Celery Worker & Beat
   - Next.js 前端
   - Docker 中间件

5. ✅ 数据库迁移
   - 从版本 391923893f21 升级到 a1b2c3d4e5f7
   - 添加了 app lifecycle 相关字段
   - Flask API 已重启以加载新的数据库架构

## ⚠️ 注意事项

### SSL 证书问题
- UAT 环境（uat-filebay.cheersai.cloud）使用自签名证书
- 脱敏程序已配置 `danger_accept_invalid_certs(true)` 处理此问题
- PowerShell/curl 直接测试可能失败，这是正常的

### uv 路径
如需永久添加 uv 到 PATH：
```powershell
# 添加到用户环境变量
[Environment]::SetEnvironmentVariable("Path", $env:Path + ";C:\Users\33814\.local\bin", "User")
```

### 可选依赖
以下依赖为可选，不影响核心功能：
- `pyOpenSSL` - SSL 连接增强
- `python-magic-bin` - MIME 类型检测

安装命令：
```bash
cd e:\CheersAI-Desktop\api
uv pip install pyopenssl python-magic-bin
```

## 🎯 下一步操作

### 测试 FileBay 连接
1. 在脱敏程序界面测试 FileBay 连接
2. 如果连接成功，可以测试文件上传功能
3. 如果连接失败，检查错误信息并排查

### 测试文件脱敏和上传
1. 在脱敏程序中选择文件进行脱敏
2. 脱敏完成后，测试上传到 FileBay
3. 在 FileBay 界面验证文件是否成功上传

### 测试 Dify 功能
1. 访问 http://localhost:3000
2. 登录 Dify 系统
3. 测试文档上传、知识库、对话等功能

## 📞 故障排查

### 如果 FileBay 连接失败
1. 检查配置文件是否存在：`filebay-config.json`
2. 检查 Vault API 是否运行：http://localhost:7788/api/v1/health
3. 检查 Token 是否有效（可能已过期）
4. 在脱敏程序界面查看详细错误信息

### 如果 Flask API 无法启动
1. 检查端口 5001 是否被占用
2. 检查 Docker 容器是否冲突
3. 查看进程日志：Terminal 7

### 如果 Celery Worker 无法处理任务
1. 检查 Redis 是否运行
2. 检查队列配置是否正确
3. 查看 Docker 容器日志：`docker logs docker-worker-1`

## 📊 服务端口总览

| 服务 | 端口 | 状态 | 访问地址 |
|------|------|------|----------|
| 脱敏程序 (Tauri) | 1420 | ✅ | http://localhost:1420/ |
| Vault API | 7788 | ✅ | http://localhost:7788/ |
| Flask API | 5001 | ✅ | http://localhost:5001/ |
| Next.js 前端 | 3000 | ✅ | http://localhost:3000/ |
| PostgreSQL | 5432 | ✅ | localhost:5432 |
| Redis | 5700 | ✅ | localhost:5700 |
| Weaviate | 8080 | ✅ | http://localhost:8080/ |
| Plugin Daemon | 5002-5003 | ✅ | http://localhost:5002/ |
| SSRF Proxy | 3128, 8194 | ✅ | - |
| Unstructured API | 8000 | ✅ | http://localhost:8000/ |
| SSO (Casdoor) | 18000 | ✅ | http://localhost:18000/ |
| SSO Database | 13306 | ✅ | localhost:13306 |
| Next.js Debugger | 9229 | ✅ | ws://127.0.0.1:9229 |

## ✅ 总结

所有必需的服务都已成功启动并运行正常！

- ✅ 脱敏程序运行正常
- ✅ Vault API 服务器运行正常
- ✅ Dify 后端 API 运行正常
- ✅ Dify 前端运行正常
- ✅ 所有 Docker 中间件运行正常
- ✅ FileBay 配置已加载

现在可以开始测试 FileBay 连接和文件上传功能了！
