/**
 * Vault 集成工具函数
 * 
 * 从 Vault Bridge 数据库读取 FileBay 配置
 */

import { invoke } from '@tauri-apps/api/core';

export interface VaultFileBayConfig {
  user_id: string;
  url: string;
  username: string;
  repo_name: string;
  email: string;
  token: string;
  updated_at: string;
}

export interface VaultDbStats {
  exists: boolean;
  path: string;
  config_count: number;
  last_updated: string | null;
}

/**
 * 列出所有可用的 Vault 配置
 */
export async function listVaultConfigs(): Promise<VaultFileBayConfig[]> {
  return await invoke<VaultFileBayConfig[]>('list_vault_configs');
}

/**
 * 通过用户 ID 获取配置
 */
export async function getVaultConfigByUserId(userId: string): Promise<VaultFileBayConfig> {
  return await invoke<VaultFileBayConfig>('get_vault_config_by_user_id', { userId });
}

/**
 * 通过邮箱获取配置
 */
export async function getVaultConfigByEmail(email: string): Promise<VaultFileBayConfig> {
  return await invoke<VaultFileBayConfig>('get_vault_config_by_email', { email });
}

/**
 * 检查 Vault 数据库是否存在
 */
export async function checkVaultDbExists(): Promise<boolean> {
  return await invoke<boolean>('check_vault_db_exists');
}

/**
 * 获取 Vault 数据库路径
 */
export async function getVaultDbPath(): Promise<string> {
  return await invoke<string>('get_vault_db_path_string');
}

/**
 * 获取 Vault 数据库统计信息
 */
export async function getVaultDbStats(): Promise<VaultDbStats> {
  return await invoke<VaultDbStats>('get_vault_db_stats');
}
