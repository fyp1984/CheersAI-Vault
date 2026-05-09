/**
 * 配置同步工具
 * 
 * 从 Desktop 在线工作区同步配置到本地
 */

import { invoke } from '@tauri-apps/api/core';
import type { SyncConfigRequest } from '@/types/commands';

/**
 * 从 Desktop 在线工作区同步配置
 * 
 * 这个函数可以在开发者控制台中调用，用于手动同步配置
 * 
 * @example
 * ```javascript
 * // 在开发者控制台中执行：
 * window.syncConfigFromDesktop({
 *   url: 'https://uat-filebay.cheersai.cloud',
 *   username: 'your_username',
 *   repo_name: 'workspace',
 *   email: 'your@email.com',
 *   token: 'your_token',
 *   user_id: 'optional_user_id'
 * })
 * ```
 */
export async function syncConfigFromDesktop(config: SyncConfigRequest): Promise<string> {
  return await invoke<string>('sync_config_from_desktop', { config });
}

/**
 * 从 Desktop 在线工作区的 localStorage 读取配置并同步
 * 
 * 这个函数需要在 Desktop 在线工作区的页面上下文中执行
 * 
 * @example
 * ```javascript
 * // 在 Desktop 在线工作区的开发者控制台中执行：
 * window.syncConfigFromLocalStorage()
 * ```
 */
export async function syncConfigFromLocalStorage(): Promise<string> {
  // 尝试从 localStorage 读取配置
  const userStr = localStorage.getItem('user');
  const filebayConfigStr = localStorage.getItem('filebay_config');
  
  if (!userStr && !filebayConfigStr) {
    throw new Error('未找到用户配置，请确保已登录');
  }
  
  let config: SyncConfigRequest;
  
  if (filebayConfigStr) {
    // 从 filebay_config 读取
    const filebayConfig = JSON.parse(filebayConfigStr);
    config = {
      url: filebayConfig.url || 'https://uat-filebay.cheersai.cloud',
      username: filebayConfig.username,
      repo_name: filebayConfig.repo_name || 'workspace',
      email: filebayConfig.email,
      token: filebayConfig.token,
      user_id: filebayConfig.user_id,
    };
  } else if (userStr) {
    // 从 user 对象读取
    const user = JSON.parse(userStr);
    config = {
      url: 'https://uat-filebay.cheersai.cloud',
      username: user.username || user.name,
      repo_name: 'workspace',
      email: user.email,
      token: user.token || '',
      user_id: user.id,
    };
  } else {
    throw new Error('配置格式不正确');
  }
  
  // 验证必需字段
  if (!config.username || !config.email) {
    throw new Error('配置不完整：缺少用户名或邮箱');
  }
  
  return await syncConfigFromDesktop(config);
}

/**
 * 从 Desktop 在线工作区的 API 获取当前用户配置并同步
 * 
 * 这个函数会调用 Desktop 的 API 获取当前登录用户的信息
 * 
 * @example
 * ```javascript
 * // 在 Desktop 在线工作区的开发者控制台中执行：
 * window.syncConfigFromAPI()
 * ```
 */
export async function syncConfigFromAPI(): Promise<string> {
  try {
    // 获取当前用户信息
    const profileResponse = await fetch('http://localhost:5001/console/api/account/profile', {
      credentials: 'include',
    });
    
    if (!profileResponse.ok) {
      throw new Error(`获取用户信息失败: ${profileResponse.status} ${profileResponse.statusText}`);
    }
    
    const profileData = await profileResponse.json();
    console.log('User profile:', profileData);
    
    // 获取 FileBay 配置
    const giteaResponse = await fetch('http://localhost:5001/console/api/account/gitea-settings', {
      credentials: 'include',
    });
    
    let giteaConfig: any = null;
    if (giteaResponse.ok) {
      giteaConfig = await giteaResponse.json();
      console.log('Gitea config:', giteaConfig);
    }
    
    // 构建配置对象
    const config: SyncConfigRequest = {
      url: giteaConfig?.url || 'https://uat-filebay.cheersai.cloud',
      username: giteaConfig?.username || profileData.name || profileData.email?.split('@')[0] || '',
      repo_name: giteaConfig?.repo_name || 'workspace',
      email: profileData.email || '',
      token: giteaConfig?.token || '',
      user_id: profileData.id,
    };
    
    // 验证必需字段
    if (!config.username || !config.email) {
      throw new Error('配置不完整：缺少用户名或邮箱');
    }
    
    if (!config.token) {
      throw new Error('未找到 Access Token，请先在 FileBay 设置中配置');
    }
    
    console.log('Syncing config:', config);
    return await syncConfigFromDesktop(config);
  } catch (error: any) {
    console.error('Failed to sync config from API:', error);
    throw new Error(`从 API 同步配置失败: ${error.message}`);
  }
}

// 将函数暴露到全局 window 对象，方便在开发者控制台中调用
if (typeof window !== 'undefined') {
  (window as any).syncConfigFromDesktop = syncConfigFromDesktop;
  (window as any).syncConfigFromLocalStorage = syncConfigFromLocalStorage;
  (window as any).syncConfigFromAPI = syncConfigFromAPI;
  
  // 添加帮助信息
  (window as any).showSyncHelp = () => {
    console.log(`
%c配置同步帮助
%c
可用的同步方法：

1. %csyncConfigFromAPI()%c (推荐)
   从 Desktop API 自动获取当前用户配置并同步
   
2. %csyncConfigFromLocalStorage()%c
   从 localStorage 读取配置并同步
   
3. %csyncConfigFromDesktop(config)%c
   手动指定配置并同步
   
示例：
%c
// 方法 1：自动从 API 获取（最简单）
await syncConfigFromAPI()

// 方法 2：从 localStorage 读取
await syncConfigFromLocalStorage()

// 方法 3：手动指定
await syncConfigFromDesktop({
  url: 'https://uat-filebay.cheersai.cloud',
  username: 'your_username',
  repo_name: 'workspace',
  email: 'your@email.com',
  token: 'your_token'
})
%c
`, 
      'color: #2563eb; font-size: 16px; font-weight: bold;',
      'color: #6b7280;',
      'color: #059669; font-weight: bold;', 'color: #6b7280;',
      'color: #059669; font-weight: bold;', 'color: #6b7280;',
      'color: #059669; font-weight: bold;', 'color: #6b7280;',
      'color: #374151; background: #f3f4f6; padding: 10px; border-radius: 4px; font-family: monospace;',
      'color: #6b7280;'
    );
  };
  
  // 启动时显示帮助
  console.log('%c💡 提示：输入 showSyncHelp() 查看配置同步帮助', 'color: #2563eb; font-weight: bold;');
}
