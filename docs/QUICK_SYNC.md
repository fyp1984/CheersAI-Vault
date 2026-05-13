# 快速配置同步指南

## 🚀 一键同步（最简单）

### 步骤：

1. **在 Desktop 在线工作区登录你的账号**
   - 访问：http://localhost:3000
   - 确保右上角显示的是正确的用户

2. **打开开发者控制台**
   - 按 `F12` 键

3. **复制下面的代码，粘贴到控制台并回车**

```javascript
(async function() {
  console.log('🔄 开始同步配置...');
  try {
    // 获取用户信息
    const profile = await fetch('http://localhost:5001/console/api/account/profile', {credentials: 'include'}).then(r => r.json());
    console.log('✅ 用户:', profile.email);
    
    // 获取 FileBay 配置
    let gitea = null;
    try {
      gitea = await fetch('http://localhost:5001/console/api/account/gitea-settings', {credentials: 'include'}).then(r => r.json());
    } catch(e) {}
    
    // 构建配置
    const config = {
      url: gitea?.url || 'https://uat-filebay.cheersai.cloud',
      username: gitea?.username || profile.name || profile.email?.split('@')[0],
      repo_name: gitea?.repo_name || 'workspace',
      email: profile.email,
      token: gitea?.token || '',
      user_id: profile.id
    };
    
    console.log('📦 配置:', {username: config.username, email: config.email, repo: config.repo_name});
    
    // 同步到 Vault API
    const result = await fetch('http://localhost:7788/api/v1/filebay/config', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify(config)
    }).then(r => r.json());
    
    console.log('✅ 同步成功!', result);
    alert(`配置同步成功！\n\n用户: ${config.username}\n邮箱: ${config.email}\n仓库: ${config.repo_name}\n\n请返回脱敏程序刷新 FileBay 设置页面。`);
  } catch(error) {
    console.error('❌ 同步失败:', error);
    alert('同步失败: ' + error.message);
  }
})();
```

4. **等待同步完成**
   - 控制台会显示同步进度
   - 成功后会弹出提示框

5. **返回脱敏程序验证**
   - 访问：http://localhost:1420/
   - 进入 "FileBay 设置" 页面
   - 点击 "刷新状态" 按钮
   - 确认用户名和邮箱已更新

---

## 📝 注意事项

- 如果提示 "未找到 Access Token"，需要先在 Desktop 的 FileBay 设置中配置
- 同步后需要在脱敏程序中点击 "刷新状态" 才能看到更新
- 切换账号后务必重新同步配置

## 🔧 故障排查

### 问题：提示 "获取用户信息失败"
**解决方案**：确保已在 Desktop 在线工作区登录

### 问题：提示 "同步到 Vault API 失败"
**解决方案**：
1. 检查脱敏程序是否正在运行
2. 检查 Vault API 是否启动：访问 http://localhost:7788/api/v1/health
3. 如果 Vault API 未启动，重启脱敏程序

### 问题：同步成功但脱敏程序中配置未更新
**解决方案**：
1. 在 FileBay 设置页面点击 "刷新状态"
2. 如果还是没有更新，重启脱敏程序

---

## 📖 详细文档

更多同步方法和详细说明，请查看：`CONFIG_SYNC_GUIDE.md`
