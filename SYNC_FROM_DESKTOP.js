/**
 * 从 Desktop 在线工作区同步配置到脱敏程序
 * 
 * 使用方法：
 * 1. 在 Desktop 在线工作区（http://localhost:3000）登录
 * 2. 打开开发者控制台（F12）
 * 3. 复制并粘贴这整个脚本到控制台
 * 4. 执行 syncConfig()
 */

async function syncConfig() {
  console.log('%c🔄 开始同步配置...', 'color: #2563eb; font-size: 14px; font-weight: bold;');
  
  try {
    // 1. 获取当前用户信息
    console.log('📡 正在获取用户信息...');
    const profileResponse = await fetch('http://localhost:5001/console/api/account/profile', {
      credentials: 'include',
    });
    
    if (!profileResponse.ok) {
      throw new Error(`获取用户信息失败: ${profileResponse.status} ${profileResponse.statusText}`);
    }
    
    const profileData = await profileResponse.json();
    console.log('✅ 用户信息:', profileData);
    
    // 2. 获取 FileBay 配置
    console.log('📡 正在获取 FileBay 配置...');
    const giteaResponse = await fetch('http://localhost:5001/console/api/account/gitea-settings', {
      credentials: 'include',
    });
    
    let giteaConfig = null;
    if (giteaResponse.ok) {
      giteaConfig = await giteaResponse.json();
      console.log('✅ FileBay 配置:', giteaConfig);
    } else {
      console.warn('⚠️ 未找到 FileBay 配置，将使用默认值');
    }
    
    // 3. 构建配置对象
    const config = {
      url: giteaConfig?.url || 'https://uat-filebay.cheersai.cloud',
      username: giteaConfig?.username || profileData.name || profileData.email?.split('@')[0] || '',
      repo_name: giteaConfig?.repo_name || 'workspace',
      email: profileData.email || '',
      token: giteaConfig?.token || '',
      user_id: profileData.id,
    };
    
    console.log('📦 配置对象:', {
      url: config.url,
      username: config.username,
      repo_name: config.repo_name,
      email: config.email,
      token: config.token ? '***已设置***' : '未设置',
      user_id: config.user_id,
    });
    
    // 4. 验证必需字段
    if (!config.username || !config.email) {
      throw new Error('配置不完整：缺少用户名或邮箱');
    }
    
    if (!config.token) {
      console.warn('⚠️ 警告：未找到 Access Token');
      console.log('💡 提示：请先在 Desktop 的 FileBay 设置中配置 Access Token');
      
      // 询问是否继续
      const continueWithoutToken = confirm('未找到 Access Token，是否继续同步？（没有 Token 将无法上传文件到 FileBay）');
      if (!continueWithoutToken) {
        console.log('❌ 用户取消同步');
        return;
      }
    }
    
    // 5. 调用脱敏程序的 Tauri 命令同步配置
    console.log('📡 正在同步到脱敏程序...');
    
    // 由于我们在 Desktop 的页面中，无法直接调用 Tauri 命令
    // 我们需要通过 Vault API 来同步
    const vaultApiResponse = await fetch('http://localhost:7788/api/v1/filebay/config', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(config),
    });
    
    if (!vaultApiResponse.ok) {
      const errorText = await vaultApiResponse.text();
      throw new Error(`同步到 Vault API 失败: ${vaultApiResponse.status} - ${errorText}`);
    }
    
    const vaultResult = await vaultApiResponse.json();
    console.log('✅ Vault API 响应:', vaultResult);
    
    // 6. 更新本地数据库和配置文件
    console.log('📝 正在更新本地配置...');
    
    // 更新 Vault Bridge 数据库
    await updateVaultDatabase(config);
    
    // 显示成功消息
    console.log('%c✅ 配置同步成功！', 'color: #059669; font-size: 16px; font-weight: bold;');
    console.log(`
%c配置详情：
%c用户: ${config.username} (${config.email})
仓库: ${config.repo_name}
服务器: ${config.url}
Token: ${config.token ? '已设置' : '未设置'}

%c下一步：
1. 返回脱敏程序（http://localhost:1420/）
2. 进入 "FileBay 设置" 页面
3. 点击 "刷新状态" 按钮
4. 确认配置已更新
`, 
      'color: #2563eb; font-weight: bold;',
      'color: #374151;',
      'color: #2563eb; font-weight: bold;'
    );
    
    alert(`配置同步成功！\n\n用户: ${config.username}\n邮箱: ${config.email}\n仓库: ${config.repo_name}\n\n请返回脱敏程序刷新页面。`);
    
  } catch (error) {
    console.error('%c❌ 同步失败', 'color: #dc2626; font-size: 14px; font-weight: bold;');
    console.error(error);
    alert(`配置同步失败：${error.message}\n\n请查看控制台了解详细信息。`);
  }
}

async function updateVaultDatabase(config) {
  // 这个函数尝试更新 Vault Bridge 数据库
  // 由于我们在浏览器环境中，无法直接访问 SQLite 数据库
  // 所以我们只能通过 Vault API 来更新
  console.log('💾 配置已通过 Vault API 保存');
}

// 显示帮助信息
function showHelp() {
  console.log(`
%c📖 配置同步帮助
%c
使用方法：
1. 确保已在 Desktop 在线工作区登录
2. 在控制台中执行：%csyncConfig()%c
3. 等待同步完成
4. 返回脱敏程序刷新页面

故障排查：
- 如果提示 "syncConfig is not defined"，请重新粘贴整个脚本
- 如果提示 "获取用户信息失败"，请确保已登录
- 如果提示 "未找到 Access Token"，请先在 FileBay 设置中配置

查看帮助：%cshowHelp()%c
`, 
    'color: #2563eb; font-size: 16px; font-weight: bold;',
    'color: #6b7280;',
    'color: #059669; font-weight: bold;', 'color: #6b7280;',
    'color: #059669; font-weight: bold;', 'color: #6b7280;'
  );
}

// 启动时显示提示
console.log('%c✨ 配置同步脚本已加载', 'color: #2563eb; font-size: 14px; font-weight: bold;');
console.log('%c💡 执行 syncConfig() 开始同步配置', 'color: #6b7280;');
console.log('%c💡 执行 showHelp() 查看帮助', 'color: #6b7280;');
