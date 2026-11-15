import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import { uploadToGitea, uploadBatchToGitea, getGiteaStatus, deleteFromGitea } from '../../services/gitea';
import { tauriCommands } from '@/lib/tauri';
import { useFileStore } from '../../store/fileStore';
import Toast from '../common/Toast';
import { FolderOpen } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

interface SandboxFile {
  name: string;
  path: string;
  size: number;
  modified: string;
}

interface ToastMessage {
  message: string;
  type: 'success' | 'error' | 'info';
}

export function FileManagerDesktop() {
  const navigate = useNavigate();
  const { outputDir } = useFileStore();
  const [files, setFiles] = useState<SandboxFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedFiles, setSelectedFiles] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState('');
  const [giteaEnabled, setGiteaEnabled] = useState(false);
  const [giteaEndpoint, setGiteaEndpoint] = useState('FileBay HTTPS 服务');
  const [giteaOwner, setGiteaOwner] = useState('');
  const [giteaRepo, setGiteaRepo] = useState('');
  const [uploading, setUploading] = useState(false);
  // 以当前目录文件的真实本机路径为键，映射到后端确认过的正式历史 ID；
  // 不再按文件名字符串匹配，避免同名普通文件被误判为可上传产物（F5）。
  const [uploadCandidates, setUploadCandidates] = useState<Record<string, string>>({});
  const [toast, setToast] = useState<ToastMessage | null>(null);
  const [confirmDialog, setConfirmDialog] = useState<{
    open: boolean;
    title: string;
    description: string;
    onConfirm: () => void;
    showCloudOption?: boolean;
    deleteCloud?: boolean;
    // C1：上传确认弹窗需要展示的完整目标信息——安全文件名/远程路径清单、
    // 目标域名与 owner/repo；只在上传相关弹窗中出现，删除/清空弹窗不使用。
    uploadDetails?: {
      files: Array<{ name: string; remotePath: string }>;
      domain: string;
      owner: string;
      repo: string;
    };
  }>({ open: false, title: '', description: '', onConfirm: () => {}, showCloudOption: false, deleteCloud: false });

  // 使用 ref 来存储最新的 deleteCloud 值
  const deleteCloudRef = useRef(false);

  useEffect(() => {
    if (outputDir) {
      loadFiles();
    } else {
      setFiles([]);
      setUploadCandidates({});
    }
    checkGiteaStatus();
  }, [outputDir]);

  const loadUploadCandidates = async (paths: string[]) => {
    if (paths.length === 0) {
      setUploadCandidates({});
      return;
    }
    try {
      setUploadCandidates(await tauriCommands.confirmFilebayUploadCandidates(paths));
    } catch {
      setUploadCandidates({});
    }
  };

  const loadFiles = async () => {
    if (!outputDir) {
      setFiles([]);
      setUploadCandidates({});
      setLoading(false);
      return;
    }

    try {
      setLoading(true);
      const result = await invoke<SandboxFile[]>('list_files_in_directory', { directory: outputDir });

      // 过滤掉旧版 .cmap 对照文件（保留 Excel 专属 .ecmap 和 .encrypted_src）
      const filteredFiles = result.filter((file) => !file.name.endsWith('.cmap'));

      // 按修改时间降序排序（最新的文件在最上面）
      filteredFiles.sort((a, b) => {
        const dateA = new Date(a.modified).getTime();
        const dateB = new Date(b.modified).getTime();
        return dateB - dateA; // 降序：最新的在前
      });

      setFiles(filteredFiles);
      void loadUploadCandidates(filteredFiles.map(file => file.path));
    } catch (error) {
      console.error('❌ 读取文件失败');
      setFiles([]);
      setUploadCandidates({});
    } finally {
      setLoading(false);
    }
  };

  const checkGiteaStatus = async () => {
    try {
      const status = await getGiteaStatus();
      setGiteaEnabled(status.enabled && status.configured && status.repo_exists === true);
      setGiteaEndpoint(status.config.url);
      setGiteaOwner(status.config.owner);
      setGiteaRepo(status.config.repo);
    } catch (error) {
      console.error('Failed to check Gitea status:', error);
    }
  };

  const handleSearch = () => {
    if (!searchQuery.trim()) {
      loadFiles();
      return;
    }

    const filtered = files.filter(f =>
      f.name.toLowerCase().includes(searchQuery.toLowerCase())
    );
    setFiles(filtered);
  };

  const handleDelete = (filePath: string, fileName?: string) => {
    const fileNameToUse = fileName || filePath.split(/[\\/]/).pop() || filePath;

    // 重置 ref
    deleteCloudRef.current = false;

    // 创建一个执行删除的函数，它会在确认时被调用
    const executeDelete = async () => {
      try {
        // 删除本地文件
        await invoke<string>('delete_sandbox_file', { filePath });

        // 从 ref 读取最新的 deleteCloud 值
        const shouldDeleteCloud = deleteCloudRef.current;

        // 如果选择了同时删除云端文件
        if (shouldDeleteCloud && giteaEnabled) {
          try {
            const remotePath = `masked/${fileNameToUse}`;
            await deleteFromGitea(remotePath, `删除文件: ${fileNameToUse}`);
            setToast({ message: '本地和云端文件已删除', type: 'success' });
          } catch (cloudError) {
            console.error('Cloud delete failed');
            setToast({ message: '本地文件已删除，但云端删除失败', type: 'error' });
          }
        } else {
          setToast({ message: '本地文件已删除', type: 'success' });
        }

        loadFiles();
      } catch (error) {
        console.error('Delete failed');
        setToast({ message: '删除失败，请重试', type: 'error' });
      }
    };

    setConfirmDialog({
      open: true,
      title: '确认删除',
      description: `确定要删除文件「${fileNameToUse}」吗？此操作不可撤销。`,
      showCloudOption: giteaEnabled,
      deleteCloud: false,
      onConfirm: () => {
        setConfirmDialog(prev => ({ ...prev, open: false }));
        executeDelete();
      },
    });
  };

  const handleBatchDelete = () => {
    if (selectedFiles.size === 0) {
      setToast({ message: '请先选择要删除的文件', type: 'info' });
      return;
    }

    const filePaths = Array.from(selectedFiles);

    // 重置 ref
    deleteCloudRef.current = false;

    // 创建一个执行批量删除的函数
    const executeBatchDelete = async () => {
      try {
        // 删除本地文件
        const result = await invoke<string>('delete_sandbox_files', { filePaths });

        // 从 ref 读取最新的 deleteCloud 值
        const shouldDeleteCloud = deleteCloudRef.current;

        // 如果选择了同时删除云端文件
        if (shouldDeleteCloud && giteaEnabled) {
          let cloudSuccessCount = 0;
          let cloudFailCount = 0;

          for (const filePath of filePaths) {
            try {
              const fileName = filePath.split(/[\\/]/).pop() || filePath;
              const remotePath = `masked/${fileName}`;
              await deleteFromGitea(remotePath, `批量删除: ${fileName}`);
              cloudSuccessCount++;
            } catch (cloudError) {
              console.error('Cloud delete failed');
              cloudFailCount++;
            }
          }

          if (cloudFailCount === 0) {
            setToast({ message: `${result}，云端文件也已删除`, type: 'success' });
          } else {
            setToast({
              message: `${result}，云端删除: ${cloudSuccessCount} 成功，${cloudFailCount} 失败`,
              type: 'error'
            });
          }
        } else {
          setToast({ message: result, type: 'success' });
        }

        setSelectedFiles(new Set());
        loadFiles();
      } catch (error) {
        console.error('Batch delete failed');
        setToast({ message: '批量删除失败，请重试', type: 'error' });
      }
    };

    setConfirmDialog({
      open: true,
      title: '批量删除确认',
      description: `确定要删除选中的 ${selectedFiles.size} 个文件吗？此操作不可撤销。`,
      showCloudOption: giteaEnabled,
      deleteCloud: false,
      onConfirm: () => {
        setConfirmDialog(prev => ({ ...prev, open: false }));
        executeBatchDelete();
      },
    });
  };

  const performUpload = async (file: SandboxFile, historyId: string) => {
    try {
      setUploading(true);
      const result = await uploadToGitea(historyId, `masked/${file.name}`, 'CheersAI Vault deidentified artifact upload');
      if (!result.success) throw new Error(result.message);
      setToast({ message: `${file.name} 已上传`, type: 'success' });
    } catch (error) {
      console.error('Upload failed');
      setToast({ message: `${file.name} 上传失败，请重试`, type: 'error' });
    } finally {
      setUploading(false);
    }
  };

  const handleUploadToGitea = async (file: SandboxFile) => {
    if (!giteaEnabled) {
      setToast({ message: '请先在 FileBay 设置中完成配置', type: 'info' });
      return;
    }

    const historyId = uploadCandidates[file.path];
    if (!historyId) {
      setToast({ message: '该文件不是已成功处理的脱敏产物，已拒绝上传', type: 'error' });
      return;
    }
    setConfirmDialog({
      open: true,
      title: '确认上传到 FileBay',
      description: '确认上传以下已完成脱敏产物？',
      uploadDetails: {
        files: [{ name: file.name, remotePath: `masked/${file.name}` }],
        domain: giteaEndpoint,
        owner: giteaOwner,
        repo: giteaRepo,
      },
      onConfirm: () => {
        setConfirmDialog(prev => ({ ...prev, open: false }));
        void performUpload(file, historyId);
      },
    });
  };


  const handleClearAll = async () => {
    if (!outputDir) {
      setToast({ message: '请先设置输出目录', type: 'error' });
      return;
    }

    const filePaths = files.map(f => f.path);
    if (filePaths.length === 0) {
      setToast({ message: '目录已经是空的', type: 'info' });
      return;
    }

    setConfirmDialog({
      open: true,
      title: '清空目录确认',
      description: `确定要清空输出目录中的 ${filePaths.length} 个文件吗？此操作不可撤销！`,
      onConfirm: async () => {
        setConfirmDialog(prev => ({ ...prev, open: false }));
        try {
          const result = await invoke<string>('delete_sandbox_files', { filePaths });
          loadFiles();
          setToast({ message: result, type: 'success' });
        } catch (error) {
          console.error('Clear failed');
          setToast({ message: '清空失败，请重试', type: 'error' });
        }
      },
    });
  };

  const handleSyncToFileBay = async () => {
    if (!giteaEnabled) {
      setToast({ message: '请先在 FileBay 设置中完成配置', type: 'info' });
      return;
    }

    if (files.length === 0) {
      setToast({ message: '没有可同步的文件', type: 'info' });
      return;
    }

    const candidateFiles = files.filter(file => uploadCandidates[file.path]);
    const pairs = candidateFiles.map(file => [uploadCandidates[file.path], `masked/${file.name}`] as [string, string]);
    if (pairs.length === 0) {
      setToast({ message: '没有可上传的已成功处理脱敏产物', type: 'error' });
      return;
    }
    setConfirmDialog({
      open: true,
      title: '确认批量上传到 FileBay',
      description: `将上传以下 ${pairs.length} 个已完成脱敏产物；其他文件不会上传。确认继续吗？`,
      uploadDetails: {
        files: candidateFiles.map(file => ({ name: file.name, remotePath: `masked/${file.name}` })),
        domain: giteaEndpoint,
        owner: giteaOwner,
        repo: giteaRepo,
      },
      onConfirm: () => {
        setConfirmDialog(prev => ({ ...prev, open: false }));
        setUploading(true);
        void uploadBatchToGitea(pairs, 'CheersAI Vault deidentified artifact upload')
          .then(result => {
            const successCount = result.items.filter(item => item.success).length;
            setToast({ message: `FileBay 上传完成：${successCount}/${pairs.length} 成功`, type: result.success ? 'success' : 'error' });
          })
          .catch(error => {
            console.error('Sync to FileBay failed');
            setToast({ message: 'FileBay 上传失败，请重试', type: 'error' });
          })
          .finally(() => setUploading(false));
      },
    });
  };

  const toggleFileSelection = (path: string) => {
    const newSelection = new Set(selectedFiles);
    if (newSelection.has(path)) {
      newSelection.delete(path);
    } else {
      newSelection.add(path);
    }
    setSelectedFiles(newSelection);
  };

  const toggleSelectAll = () => {
    if (selectedFiles.size === files.length) {
      setSelectedFiles(new Set());
    } else {
      setSelectedFiles(new Set(files.map(f => f.path)));
    }
  };

  const formatFileSize = (bytes: number): string => {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(2) + ' KB';
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(2) + ' MB';
    return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
  };


  if (!outputDir) {
    return (
      <div className="p-6 max-w-7xl mx-auto">
        <div className="mb-6">
          <h2 className="text-2xl font-bold text-gray-900 mb-2">文件管理</h2>
          <p className="text-gray-600">管理输出目录中的脱敏文件</p>
        </div>
        <div className="flex flex-col items-center justify-center py-20 bg-gray-50 border-2 border-dashed border-gray-200 rounded-xl">
          <FolderOpen className="w-16 h-16 text-gray-400 mb-4" />
          <h3 className="text-lg font-semibold text-gray-700 mb-2">尚未配置输出目录</h3>
          <p className="text-sm text-gray-500 mb-6">请先前往「文件脱敏」页面，点击「选择输出目录」完成配置</p>
          <button
            onClick={() => navigate('/process')}
            className="px-4 py-2 bg-blue-500 text-white text-sm rounded-md hover:bg-blue-600 transition-colors"
          >
            前往文件脱敏
          </button>
        </div>
      </div>
    );
  }

  if (loading && files.length === 0) {
    return (
      <div className="p-6">
        <div className="animate-pulse space-y-4">
          <div className="h-8 bg-gray-200 rounded w-1/4"></div>
          <div className="h-64 bg-gray-200 rounded"></div>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 max-w-7xl mx-auto">
      {/* 标题和统计 */}
      <div className="mb-6">
        <h2 className="text-2xl font-bold text-gray-900 mb-2">文件管理</h2>
        <p className="text-gray-600">管理输出目录中的脱敏文件</p>
        <p className="text-sm text-gray-500 mt-1">
          输出目录: <code className="bg-gray-100 px-2 py-1 rounded">{outputDir}</code>
        </p>
      </div>

      {/* 统计卡片 */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
        <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
          <div className="text-sm text-blue-600 font-medium">文件总数</div>
          <div className="text-2xl font-bold text-blue-900">{files.length}</div>
        </div>
        <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
          <div className="text-sm text-blue-600 font-medium">总大小</div>
          <div className="text-2xl font-bold text-blue-900">
            {formatFileSize(files.reduce((sum, f) => sum + f.size, 0))}
          </div>
        </div>
      </div>

      {/* 搜索和操作栏 */}
      <div className="mb-4 flex items-center justify-between gap-4 flex-wrap">
        <div className="flex-1 flex items-center gap-2 min-w-[300px]">
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyPress={(e) => e.key === 'Enter' && handleSearch()}
            placeholder="搜索文件名..."
            className="flex-1 px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          <button
            onClick={handleSearch}
            className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700"
          >
            搜索
          </button>
          {searchQuery && (
            <button
              onClick={() => {
                setSearchQuery('');
                loadFiles();
              }}
              className="px-4 py-2 border border-gray-300 text-gray-700 rounded-md hover:bg-gray-50"
            >
              清除
            </button>
          )}
        </div>

        <div className="flex items-center gap-2">
          {files.length > 0 && (
            <button
              onClick={handleSyncToFileBay}
              disabled={uploading}
              className="px-3 py-2 bg-blue-600 text-white text-sm rounded-md hover:bg-blue-700 disabled:opacity-50 flex items-center gap-1"
            >
              {uploading ? (
                <>
                  <svg className="animate-spin h-4 w-4" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                  </svg>
                  同步中...
                </>
              ) : (
                <>
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                  </svg>
                  一键同步
                </>
              )}
            </button>
          )}
          <button
            onClick={loadFiles}
            className="px-3 py-2 border border-gray-300 text-gray-700 text-sm rounded-md hover:bg-gray-50"
          >
            刷新
          </button>
          {selectedFiles.size > 0 && (
            <>
              <span className="text-sm text-gray-600">已选择 {selectedFiles.size} 个</span>
              <button
                onClick={handleBatchDelete}
                className="px-3 py-2 bg-red-600 text-white text-sm rounded-md hover:bg-red-700"
              >
                批量删除
              </button>
            </>
          )}
          {files.length > 0 && (
            <button
              onClick={handleClearAll}
              className="px-3 py-2 bg-red-700 text-white text-sm rounded-md hover:bg-red-800"
            >
              清空目录
            </button>
          )}
        </div>
      </div>

      {/* 文件列表 */}
      <div className="bg-white border border-gray-200 rounded-lg overflow-hidden">
        <table className="min-w-full divide-y divide-gray-200">
          <thead className="bg-gray-50">
            <tr>
              <th className="px-4 py-3 text-left w-12">
                <input
                  type="checkbox"
                  checked={selectedFiles.size === files.length && files.length > 0}
                  onChange={toggleSelectAll}
                  className="rounded border-gray-300"
                />
              </th>
              <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider w-auto">
                文件名
              </th>
              <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider w-32">
                大小
              </th>
              <th className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider w-32">
                操作
              </th>
            </tr>
          </thead>
          <tbody className="bg-white divide-y divide-gray-200">
            {files.length === 0 ? (
              <tr>
                <td colSpan={4} className="px-4 py-8 text-center text-gray-500">
                  暂无文件。脱敏后的文件会自动保存到沙箱目录。
                </td>
              </tr>
            ) : (
              files.map((file) => (
                <tr key={file.path} className="hover:bg-gray-50">
                  <td className="px-4 py-3">
                    <input
                      type="checkbox"
                      checked={selectedFiles.has(file.path)}
                      onChange={() => toggleFileSelection(file.path)}
                      className="rounded border-gray-300"
                    />
                  </td>
                  <td className="px-4 py-3 text-sm text-gray-900">
                    <div className="flex items-center gap-2 min-w-0 flex-wrap">
                      {(file.name.includes('masked') || file.name.includes('_脱敏')) && (
                        <span className="inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium bg-blue-100 text-blue-800 flex-shrink-0">
                          脱敏
                        </span>
                      )}
                      {file.name.endsWith('.ecmap') && (
                        <Badge className="bg-blue-100 text-blue-700 border-blue-200 flex-shrink-0">
                          映射对照 v1.2
                        </Badge>
                      )}
                      {file.name.endsWith('.encrypted_src') && (
                        <Badge className="bg-purple-100 text-purple-700 border-purple-200 flex-shrink-0">
                          加密源
                        </Badge>
                      )}
                      <span
                        className="truncate max-w-xl"
                        title={file.name}
                      >
                        {file.name}
                      </span>
                    </div>
                  </td>
                  <td className="px-4 py-3 text-sm text-gray-500 w-32">
                    {formatFileSize(file.size)}
                  </td>
                  <td className="px-4 py-3 text-sm w-32">
                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => handleUploadToGitea(file)}
                        disabled={uploading || !uploadCandidates[file.path]}
                        className={`${giteaEnabled && uploadCandidates[file.path] ? 'text-blue-600 hover:text-blue-800' : 'text-gray-400'} disabled:opacity-50`}
                        title={giteaEnabled ? (uploadCandidates[file.path] ? '上传到 FileBay' : '不是本产品已完成脱敏产物') : '请先配置 FileBay'}
                      >
                        上传
                      </button>
                      <button
                        onClick={() => handleDelete(file.path)}
                        className="text-red-600 hover:text-red-800"
                      >
                        删除
                      </button>
                    </div>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* 确认删除弹窗 */}
      <Dialog open={confirmDialog.open} onOpenChange={(open) => setConfirmDialog(prev => ({ ...prev, open }))}>
        <DialogContent className="sm:max-w-[450px]">
          <DialogHeader>
            <DialogTitle>{confirmDialog.title}</DialogTitle>
            <DialogDescription>{confirmDialog.description}</DialogDescription>
          </DialogHeader>

          {/* FileBay 上传目标详情（C1：安全文件名/远程路径清单、目标域名、owner/repo） */}
          {confirmDialog.uploadDetails && (
            <div className="py-2 space-y-2 text-sm">
              <div>
                <span className="font-medium text-gray-700">目标域名：</span>
                <span className="text-gray-900">{confirmDialog.uploadDetails.domain}</span>
              </div>
              <div>
                <span className="font-medium text-gray-700">仓库：</span>
                <span className="text-gray-900">
                  {confirmDialog.uploadDetails.owner}/{confirmDialog.uploadDetails.repo}
                </span>
              </div>
              <div className="font-medium text-gray-700">上传文件清单：</div>
              <ul className="list-disc pl-5 max-h-40 overflow-auto text-gray-900">
                {confirmDialog.uploadDetails.files.map(item => (
                  <li key={item.remotePath} title={item.name}>
                    {item.name} → {item.remotePath}
                  </li>
                ))}
              </ul>
              <p className="text-xs text-gray-500">
                只上传已完成脱敏 Markdown，不上传原文或 .cmap。
              </p>
            </div>
          )}

          {/* 云端删除选项 */}
          {confirmDialog.showCloudOption && (
            <div className="py-4">
              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={confirmDialog.deleteCloud}
                  onChange={(e) => {
                    const checked = e.target.checked;
                    setConfirmDialog(prev => ({ ...prev, deleteCloud: checked }));
                    deleteCloudRef.current = checked; // 同时更新 ref
                  }}
                  className="w-4 h-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                />
                <div className="flex-1">
                  <div className="text-sm font-medium text-gray-900">同时删除 FileBay 云端文件</div>
                  <div className="text-xs text-gray-500 mt-0.5">
                    勾选此项将同时删除 FileBay 仓库中的对应文件
                  </div>
                </div>
              </label>
            </div>
          )}

          <DialogFooter className="gap-2">
            <Button variant="outline" onClick={() => setConfirmDialog(prev => ({ ...prev, open: false }))}>
              取消
            </Button>
            <Button className="bg-red-600 hover:bg-red-700 text-white" onClick={confirmDialog.onConfirm}>
              确认
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Toast 通知 */}
      {toast && (
        <Toast
          message={toast.message}
          type={toast.type}
          onClose={() => setToast(null)}
        />
      )}
    </div>
  );
}
