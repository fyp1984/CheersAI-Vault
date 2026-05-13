# 🚀 Vault 集成快速开始

## 5 分钟快速上手

### ✅ 前提条件

- [x] Vault 数据库已存在：`~/.cheersai/vault.db`
- [x] 数据库中已有配置（通过 `http://localhost:3000/sync-config` 同步）

### 📝 快速步骤

#### 1. 启动脱敏程序

```bash
cd E:\CheersAI脱敏\cheersai-desktop
pnpm tauri dev
```

#### 2. 进入 FileBay 设置

在应用中点击左侧菜单的 **"FileBay 设置"**

#### 3. 从 Vault 加载配置

1. 点击 **"从 Vault 加载"** 按钮（紫色按钮）
2. 从列表中选择你的配置
3. 配置会自动填充到表单
4. 点击 **"保存配置"** 按钮

#### 4. 完成！

现在你可以开始使用 FileBay 上传功能了！🎉

---

## 🔍 如果遇到问题

### 问题：点击"从 Vault 加载"后显示错误

#### 错误 1：数据库不存在

**解决方案**：
```bash
# 访问浏览器
http://localhost:3000/sync-config

# 点击"开始同步"按钮
```

#### 错误 2：数据库为空

**解决方案**：
```bash
# 1. 先登录
http://localhost:3000/signin

# 2. 然后同步
http://localhost:3000/sync-config
```

---

## 📊 验证配置

### 方法 1：使用测试脚本

```powershell
.\test-vault-integration.ps1
```

### 方法 2：手动检查数据库

```powershell
# 查看数据库路径
$env:USERPROFILE\.cheersai\vault.db

# 使用 SQLite 查询
sqlite3 $env:USERPROFILE\.cheersai\vault.db "SELECT email, username, repo_name FROM filebay_configs;"
```

---

## 🎯 配置方式选择

| 方式 | 何时使用 |
|------|----------|
| **从 Vault 加载** | ✅ 推荐！已在 Vault 系统中登录 |
| 读取已下载配置 | 已从 Desktop 下载配置文件 |
| 手动导入配置 | 有配置文件但不在 Downloads 文件夹 |
| 完全手动输入 | 没有其他选择时 |

---

## 📚 更多信息

- 完整指南：`VAULT_INTEGRATION_COMPLETE.md`
- 技术总结：`VAULT_INTEGRATION_SUMMARY.md`
- 实现细节：`DESENSITIZATION_APP_VAULT_INTEGRATION.md`

---

**就这么简单！开始使用吧！** 🚀
