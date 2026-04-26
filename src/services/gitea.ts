import { invoke } from '@tauri-apps/api/core';
import type { GiteaStatusResponse, UploadResult } from '../types/gitea';

/**
 * 获取 Gitea 配置状态
 */
export async function getGiteaStatus(): Promise<GiteaStatusResponse> {
  return await invoke<GiteaStatusResponse>('get_gitea_status');
}

/**
 * 更新 Gitea 配置
 */
export const updateGiteaConfig = (config: { url?: string; token?: string; owner?: string; repo?: string; enabled?: boolean }) => {
  return invoke<string>('update_gitea_config', config);
}

export const testGiteaConnection = () => {
  return invoke<string>('test_gitea_connection');
}

/**
 * 创建 Gitea 仓库
 */
export async function createGiteaRepo(isPrivate: boolean = true): Promise<string> {
  return await invoke<string>('create_gitea_repo', { private: isPrivate });
}

/**
 * 上传单个文件到 Gitea
 */
export async function uploadToGitea(
  filePath: string,
  remotePath: string,
  message?: string
): Promise<UploadResult> {
  return await invoke<UploadResult>('upload_to_gitea', {
    filePath,
    remotePath,
    message,
  });
}

/**
 * 批量上传文件到 Gitea
 */
export async function uploadBatchToGitea(
  files: Array<[string, string]>, // [本地路径, 远程路径]
  message?: string
): Promise<UploadResult> {
  return await invoke<UploadResult>('upload_batch_to_gitea', {
    files,
    message,
  });
}
