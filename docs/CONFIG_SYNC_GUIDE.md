# 配置同步指南

当你在 Desktop 在线工作区切换账号后，需要手动同步配置到本地脱敏程序。

## 快速开始（推荐方式）

**最简单的方式**：

1. 在 Desktop 在线工作区登录你的账号
2. 按 `F12` 打开开发者控制台
3. 输入并执行：
   ```javascript
   await syncConfigFromAPI()
   ```
4. 等待同步完成
5. 返回脱敏程序，刷新 FileBay 设置页面

**查看帮助**：
```javascript
showSyncHelp()
```

---

## 方法 1：从 Desktop 在线工作区 API 同步（最简单，推荐）

### 步骤：

1. **打开 Desktop 在线工作区**
   - 在脱敏程序中点击左侧菜单 "CheersAI"
   - 或直接访问：http://localhost:3000

2. **确保已登录正确的账号**
   - 检查右上角显示的用户名和邮箱
   - 如果不是目标账号，请先登出并重新登录

3. **打开开发者控制台**
   - Windows/Linux: 按 `F12` 或 `Ctrl + Shift + I`
   - macOS: 按 `Cmd + Option + I`

4. **在控制台中执行同步命令**
   
   ```javascript
   // 最简单的方式：自动从 API 获取当前用户配置
   await syncConfigFromAPI()
   ```
   
   这个命令会：
   - 自动调用 Desktop API 获取当前登录用户信息
   - 自动获取 FileBay 配置（如果已配置）
   - 同步到本地所有位置

5. **等待同步完成**
   - 控制台会显示同步结果
   - 成功后会显示：`配置同步成功！用户: xxx (xxx@xxx.com) 仓库: workspace`

6. **刷新脱敏程序的 FileBay 设置页面**
   - 返回脱敏程序
   - 进入 "FileBay 设置" 页面
   - 点击 "刷新状态" 按钮
   - 确认显示的用户名和邮箱已更新

## 方法 2：从 Desktop 在线工作区 localStorage 同步

### 步骤：

1. **打开 Desktop 在线工作区**
   - 在脱敏程序中点击左侧菜单 "CheersAI"
   - 或直接访问：http://localhost:3000

2. **确保已登录正确的账号**
   - 检查右上角显示的用户名和邮箱
   - 如果不是目标账号，请先登出并重新登录

3. **打开开发者控制台**
   - Windows/Linux: 按 `F12` 或 `Ctrl + Shift + I`
   - macOS: 按 `Cmd + Option + I`

4. **在控制台中执行同步命令**
   
   ```javascript
   // 方式 A：自动从 localStorage 读取配置（推荐）
   window.syncConfigFromLocalStorage()
   ```
   
   或者
   
   ```javascript
   // 方式 B：手动指定配置
   window.syncConfigFromDesktop({
     url: 'https://uat-filebay.cheersai.cloud',
     username: 'your_username',        // 你的用户名
     repo_name: 'workspace',            // 仓库名
     email: 'your@email.com',           // 你的邮箱
     token: 'your_access_token',        // 你的 Access Token
     user_id: 'optional_user_id'        // 可选：用户 ID
   })
   ```

5. **等待同步完成**
   - 控制台会显示同步结果
   - 成功后会显示：`配置同步成功！用户: xxx (xxx@xxx.com) 仓库: workspace`

6. **刷新脱敏程序的 FileBay 设置页面**
   - 返回脱敏程序
   - 进入 "FileBay 设置" 页面
   - 点击 "刷新状态" 按钮
   - 确认显示的用户名和邮箱已更新

## 方法 2：从 Desktop 在线工作区 localStorage 同步

### 步骤：

1. **打开 Desktop 在线工作区并登录**

2. **打开开发者控制台** (F12)

3. **在控制台中执行**
   
   ```javascript
   // 从 localStorage 读取配置
   await syncConfigFromLocalStorage()
   ```

## 方法 3：手动指定配置同步

## 方法 3：手动指定配置同步

### 步骤：

1. **打开开发者控制台** (在 Desktop 或脱敏程序中都可以)

2. **在控制台中执行**
   
   ```javascript
   await syncConfigFromDesktop({
     url: 'https://uat-filebay.cheersai.cloud',
     username: 'your_username',
     repo_name: 'workspace',
     email: 'your@email.com',
     token: 'your_access_token'
   })
   ```

## 方法 4：使用 "从 Vault 加载" 按钮

如果你已经在 Desktop 在线工作区的 "同步配置" 页面（http://localhost:3000/sync-config）完成了配置同步：

1. 进入脱敏程序的 "FileBay 设置" 页面
2. 点击 "从 Vault 加载" 按钮
3. 选择正确的配置
4. 点击 "保存配置"

## 方法 4：使用 "从 Vault 加载" 按钮

如果你已经在 Desktop 在线工作区的 "同步配置" 页面（http://localhost:3000/sync-config）完成了配置同步：

1. 进入脱敏程序的 "FileBay 设置" 页面
2. 点击 "从 Vault 加载" 按钮
3. 选择正确的配置
4. 点击 "保存配置"

## 方法 5：使用 "读取已下载配置" 按钮

如果你从 Desktop 在线工作区下载了配置文件：

1. 在 Desktop 在线工作区的 "FileBay 设置" 页面点击 "下载配置文件"
2. 文件会保存到浏览器的 Downloads 文件夹
3. 返回脱敏程序的 "FileBay 设置" 页面
4. 点击 "读取已下载配置" 按钮
5. 点击 "保存配置"

## 同步的配置位置

配置会同步到以下三个位置：

1. **Vault Bridge 数据库**: `~/.cheersai/vault.db`
   - 用于 "从 Vault 加载" 功能

2. **Gitea 配置文件**: `%TEMP%\cheersai-vault\gitea_config.json`
   - 脱敏程序实际使用的配置

3. **FileBay 配置文件**: `%LOCALAPPDATA%\com.cheersai.vault\downloads\filebay-config.json`
   - 用于 "读取已下载配置" 功能

## 故障排查

### 问题：控制台提示 "syncConfigFromAPI is not a function"

**解决方案**：
- 刷新页面后重试
- 确保在正确的页面（Desktop 在线工作区或脱敏程序）
- 输入 `showSyncHelp()` 查看可用的函数

### 问题：同步后配置没有更新

**解决方案**：
1. 在 FileBay 设置页面点击 "刷新状态"
2. 如果还是没有更新，重启脱敏程序

### 问题：提示 "未找到用户配置"

**解决方案**：
- 确保已在 Desktop 在线工作区登录
- 使用方式 B 手动指定配置

### 问题：不知道 Access Token

**解决方案**：
1. 登录 FileBay 服务器：https://uat-filebay.cheersai.cloud
2. 进入 设置 → 应用 → 管理访问令牌
3. 生成新的 Token（需要 repo 权限）
4. 复制 Token 并使用方式 B 手动同步

## 注意事项

- 同步配置需要有效的 Access Token
- Token 会以明文形式存储在本地数据库和配置文件中
- 切换账号后务必同步配置，否则可能上传到错误的仓库
- 建议定期更新 Access Token 以确保安全
