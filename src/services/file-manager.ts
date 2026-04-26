import { invoke } from '@tauri-apps/api/core';
import type { ManagedFile, FileListResponse, FileStatistics } from '../types/file-manager';

/**
 * 添加文件到管理列表
 */
export async function addManagedFile(
  originalName: string,
  maskedName: string,
  filePath: string,
  fileSize: number,
  fileType: string,
  maskedCount: number
): Promise<string> {
  return await invoke<string>('add_managed_file', {
    originalName,
    maskedName,
    filePath,
    fileSize,
    fileType,
    maskedCount,
  });
}

/**
 * 获取文件列表
 */
export async function getManagedFiles(
  page: number = 1,
  pageSize: number = 20
): Promise<FileListResponse> {
  return await invoke<FileListResponse>('get_managed_files', {
    page,
    pageSize,
  });
}

/**
 * 获取单个文件信息
 */
export async function getManagedFile(id: string): Promise<ManagedFile | null> {
  return await invoke<ManagedFile | null>('get_managed_file', { id });
}

/**
 * 更新文件信息
 */
export async function updateManagedFile(file: ManagedFile): Promise<string> {
  return await invoke<string>('update_managed_file', { file });
}

/**
 * 删除文件
 */
export async function deleteManagedFile(
  id: string,
  deletePhysical: boolean = false
): Promise<string> {
  return await invoke<string>('delete_managed_file', {
    id,
    deletePhysical,
  });
}

/**
 * 批量删除文件
 */
export async function deleteManagedFiles(
  ids: string[],
  deletePhysical: boolean = false
): Promise<string> {
  return await invoke<string>('delete_managed_files', {
    ids,
    deletePhysical,
  });
}

/**
 * 标记文件已上传
 */
export async function markFileUploaded(id: string, giteaUrl: string): Promise<string> {
  return await invoke<string>('mark_file_uploaded', {
    id,
    giteaUrl,
  });
}

/**
 * 搜索文件
 */
export async function searchManagedFiles(
  query: string,
  limit: number = 50
): Promise<ManagedFile[]> {
  return await invoke<ManagedFile[]>('search_managed_files', {
    query,
    limit,
  });
}

/**
 * 获取文件统计信息
 */
export async function getFileStatistics(): Promise<FileStatistics> {
  return await invoke<FileStatistics>('get_file_statistics');
}
