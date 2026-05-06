import { useState, useEffect } from 'react';
import { Download, CheckCircle, AlertCircle, Loader2, Package, Trash2, Brain, ExternalLink, FolderOpen } from 'lucide-react';
import { tauriCommands } from '@/lib/tauri';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import type { PlatformContext } from '@/types/commands';
import type { InstallerProgress } from '@/types/commands';

interface ServiceStatus {
  ocr: boolean;
  aiModel: boolean;
  ollamaInstalled: boolean;
  ollamaRunning: boolean;
}

export function EnhancedServices() {
  const [serviceStatus, setServiceStatus] = useState<ServiceStatus>({
    ocr: false,
    aiModel: false,
    ollamaInstalled: false,
    ollamaRunning: false,
  });
  const [loading, setLoading] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [startingOllama, setStartingOllama] = useState(false);
  const [installing, setInstalling] = useState({
    ocr: false,
    aiModel: false,
  });
  const [uninstalling, setUninstalling] = useState({
    ocr: false,
    aiModel: false,
  });
  const [downloadProgress, setDownloadProgress] = useState({
    ocr: 0,
    aiModel: 0,
  });
  const [progressStatus, setProgressStatus] = useState({
    ocr: '',
    aiModel: '',
  });
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);
  const [showPathDialog, setShowPathDialog] = useState<'ocr' | 'aiModel' | null>(null);
  const [platformContext, setPlatformContext] = useState<PlatformContext | null>(null);
  const [confirmDialog, setConfirmDialog] = useState<{
    open: boolean;
    title: string;
    description: string;
    onConfirm: () => Promise<void>;
  }>({
    open: false,
    title: '',
    description: '',
    onConfirm: async () => {},
  });
  const [customPaths, setCustomPaths] = useState({
    ocr: '',
    aiModel: '',
  });

  useEffect(() => {
    checkServicesStatus();
    tauriCommands.getPlatformContext()
      .then(setPlatformContext)
      .catch((error) => console.error('Failed to load platform context:', error));
  }, []);

  const isMac = platformContext?.os === 'macos';
  const platformLabel = platformContext?.os === 'macos' ? 'macOS' : platformContext?.os === 'windows' ? 'Windows' : 'Linux';
  const joinPlatformPath = (base: string, child: string) => {
    const separator = platformContext?.pathSeparator || '/';
    return `${base.replace(/[\\/]+$/, '')}${separator}${child}`;
  };
  const defaultInstallPlaceholder = (service: 'ocr' | 'aiModel') =>
    platformContext?.appDataDir
      ? joinPlatformPath(platformContext.appDataDir, service === 'ocr' ? 'ocr-package' : 'ollama')
      : service === 'ocr'
        ? '选择 OCR 运行时目录'
        : '选择 Ollama 目录';

  useEffect(() => {
    const unlistenPromises: Promise<() => void>[] = [];
    
    // 监听 OCR 安装进度
    unlistenPromises.push(
      listen<InstallerProgress>('ocr-install-progress', (event) => {
        console.log('OCR install progress:', event.payload);
        setDownloadProgress(prev => ({ ...prev, ocr: event.payload.percentage }));
        setProgressStatus(prev => ({ ...prev, ocr: event.payload.status }));
      })
    );

    // 监听 OCR 卸载进度
    unlistenPromises.push(
      listen<InstallerProgress>('ocr-uninstall-progress', (event) => {
        console.log('OCR uninstall progress:', event.payload);
        setDownloadProgress(prev => ({ ...prev, ocr: event.payload.percentage }));
        setProgressStatus(prev => ({ ...prev, ocr: event.payload.status }));
      })
    );

    // 监听 Ollama 安装进度
    unlistenPromises.push(
      listen<InstallerProgress>('ollama-install-progress', (event) => {
        console.log('Ollama install progress:', event.payload);
        setDownloadProgress(prev => ({ ...prev, aiModel: event.payload.percentage }));
        setProgressStatus(prev => ({ ...prev, aiModel: event.payload.status }));
      })
    );

    // 监听 Ollama 卸载进度
    unlistenPromises.push(
      listen<InstallerProgress>('ollama-uninstall-progress', (event) => {
        console.log('Ollama uninstall progress:', event.payload);
        setDownloadProgress(prev => ({ ...prev, aiModel: event.payload.percentage }));
        setProgressStatus(prev => ({ ...prev, aiModel: event.payload.status }));
      })
    );
    
    // 清理函数
    return () => {
      Promise.all(unlistenPromises).then(unlisteners => {
        unlisteners.forEach(unlisten => unlisten());
      });
    };
  }, []);

  const checkServicesStatus = async () => {
    try {
      setLoading(true);
      
      let ocrInstalled = false;
      let aiModelInstalled = false;
      let ollamaInstalled = false;
      let ollamaRunning = false;
      
      try {
        ocrInstalled = await tauriCommands.checkOcrInstalled();
      } catch (error) {
        console.error('Failed to check OCR:', error);
      }
      
      try {
        ollamaInstalled = await tauriCommands.checkOllamaBinaryInstalled();
      } catch (error) {
        console.error('Failed to check Ollama binary:', error);
      }
      
      try {
        ollamaRunning = await tauriCommands.checkOllamaServiceRunning();
      } catch (error) {
        console.error('Failed to check Ollama service:', error);
      }

      try {
        aiModelInstalled = await tauriCommands.checkAiModelInstalled();
      } catch (error) {
        console.error('Failed to check AI model:', error);
      }
      
      setServiceStatus({
        ocr: ocrInstalled,
        aiModel: aiModelInstalled,
        ollamaInstalled,
        ollamaRunning,
      });
    } finally {
      setLoading(false);
    }
  };

  const handleScanServices = async () => {
    try {
      setScanning(true);
      setMessage({ type: 'info', text: '正在扫描已安装的服务...' });
      
      // 分别检查每个服务
      let ocrInstalled = false;
      let aiModelInstalled = false;
      let ollamaInstalled = false;
      let ollamaRunning = false;
      
      try {
        ocrInstalled = await tauriCommands.checkOcrInstalled();
        console.log('OCR installed:', ocrInstalled);
      } catch (error) {
        console.error('Failed to check OCR:', error);
      }
      
      try {
        ollamaInstalled = await tauriCommands.checkOllamaBinaryInstalled();
        console.log('Ollama installed:', ollamaInstalled);
      } catch (error) {
        console.error('Failed to check Ollama:', error);
      }

      try {
        ollamaRunning = await tauriCommands.checkOllamaServiceRunning();
        console.log('Ollama service running:', ollamaRunning);
      } catch (error) {
        console.error('Failed to check Ollama service:', error);
      }
      
      try {
        aiModelInstalled = await tauriCommands.checkAiModelInstalled();
        console.log('AI Model installed:', aiModelInstalled);
      } catch (error) {
        console.error('Failed to check AI model:', error);
      }
      
      setServiceStatus({
        ocr: ocrInstalled,
        aiModel: aiModelInstalled,
        ollamaInstalled,
        ollamaRunning,
      });
      
      // 构建详细的扫描结果消息
      const installedServices = [];
      const warnings = [];
      
      if (ocrInstalled) {
        installedServices.push('OCR 服务');
      }
      
      if (aiModelInstalled) {
        installedServices.push('AI 模型 (qwen2.5:1.5b)');
      } else if (ollamaInstalled && !ollamaRunning) {
        warnings.push('Ollama 已安装但服务未启动');
      } else if (ollamaInstalled) {
        warnings.push('Ollama 已安装但 qwen2.5:1.5b 模型未安装');
      } else {
        warnings.push('Ollama 未安装');
      }
      
      // 显示结果
      if (installedServices.length > 0 || warnings.length > 0) {
        let messageText = '';
        
        if (installedServices.length > 0) {
          messageText += `已检测到: ${installedServices.join('、')}`;
        }
        
        if (warnings.length > 0) {
          if (messageText) messageText += '；';
          messageText += warnings.join('；');
        }
        
        setMessage({ 
          type: installedServices.length > 0 ? 'success' : 'info', 
          text: `扫描完成！${messageText}` 
        });
      } else {
        setMessage({ 
          type: 'info', 
          text: '扫描完成，未检测到已安装的服务' 
        });
      }
      
      return { ollamaInstalled, aiModelInstalled };
    } catch (error) {
      console.error('Failed to scan services:', error);
      setMessage({ type: 'error', text: `扫描失败: ${error}` });
      return { ollamaInstalled: false, aiModelInstalled: false };
    } finally {
      setScanning(false);
    }
  };

  const handleStartOllama = async () => {
    try {
      setStartingOllama(true);
      setMessage({ type: 'info', text: '正在启动 Ollama 服务...' });
      
      const result = await tauriCommands.startOllamaService();
      setMessage({ type: 'success', text: result + '，请稍等几秒后重新扫描' });
      
      // 等待 3 秒后自动重新扫描
      setTimeout(async () => {
        await handleScanServices();
      }, 3000);
    } catch (error) {
      console.error('Failed to start Ollama:', error);
      setMessage({ type: 'error', text: `启动失败: ${error}` });
    } finally {
      setStartingOllama(false);
    }
  };

  const handleInstallOcr = async () => {
    // 显示路径选择对话框
    setShowPathDialog('ocr');
  };

  const handleConfirmInstallOcr = async () => {
    try {
      setShowPathDialog(null);
      setInstalling((prev) => ({ ...prev, ocr: true }));
      setMessage(null);
      setDownloadProgress((prev) => ({ ...prev, ocr: 0 }));

      setMessage({ type: 'info', text: isMac ? '正在准备 macOS OCR 运行时，请稍候...' : '正在下载 OCR 包，请稍候...' });

      // 传递自定义路径（如果有）
      await tauriCommands.downloadOcrPackage(customPaths.ocr || undefined);

      setMessage({ type: 'success', text: 'OCR 运行时安装成功。现在可以进行 PDF 文本提取；若是纯图片型 PDF，请准备更完整的 OCR 环境。' });
      await checkServicesStatus();
    } catch (error) {
      console.error('Failed to install OCR:', error);
      setMessage({ type: 'error', text: `安装失败: ${error}` });
    } finally {
      setInstalling((prev) => ({ ...prev, ocr: false }));
      setDownloadProgress((prev) => ({ ...prev, ocr: 0 }));
    }
  };

  const handleUninstallOcr = async () => {
    setConfirmDialog({
      open: true,
      title: '确认卸载 OCR 运行时',
      description: '卸载后将无法进行 PDF 文本提取，需要重新安装后才能继续使用。',
      onConfirm: async () => {
        setConfirmDialog((prev) => ({ ...prev, open: false }));
        setUninstalling((prev) => ({ ...prev, ocr: true }));
        setMessage(null);

        try {
          await tauriCommands.uninstallOcrPackage();

          setMessage({ type: 'success', text: 'OCR 运行时已卸载' });
          await checkServicesStatus();
        } catch (error) {
          console.error('Failed to uninstall OCR:', error);
          setMessage({ type: 'error', text: `卸载失败: ${error}` });
        } finally {
          setUninstalling((prev) => ({ ...prev, ocr: false }));
        }
      },
    });
  };

  const handleInstallAiModel = async () => {
    // 显示路径选择对话框
    setShowPathDialog('aiModel');
  };

  const handleConfirmInstallAiModel = async () => {
    try {
      setShowPathDialog(null);
      setInstalling((prev) => ({ ...prev, aiModel: true }));
      setMessage(null);
      setDownloadProgress((prev) => ({ ...prev, aiModel: 0 }));

      setProgressStatus((prev) => ({ ...prev, aiModel: '' }));

      // 使用 Ollama 官方脚本自动安装
      setMessage({ 
        type: 'info', 
        text: '正在使用 Ollama 官方脚本安装...\n' +
              '首次安装需要下载约 1.6GB 文件，请耐心等待。\n' +
              '安装过程可能需要 5-10 分钟。'
      });

      await tauriCommands.installOllamaWithScript();

      setMessage({ type: 'success', text: 'Ollama 和 AI 模型安装成功！' });
      await checkServicesStatus();
    } catch (error) {
      console.error('Failed to install AI model:', error);
      
      // 如果安装失败，提供手动安装指引
      const errorMsg = String(error);
      let helpText = `安装失败: ${errorMsg}\n\n`;
      
      if (errorMsg.includes('Python') || errorMsg.includes('python')) {
        helpText += '提示：此功能需要 Python 3.7+ 才能使用自动安装。\n\n';
      }
      
      helpText += '您也可以手动安装 Ollama：\n' +
                  '方法1（推荐）：\n' +
                  '  1. 打开 PowerShell（管理员）\n' +
                  '  2. 运行: irm https://ollama.com/install.ps1 | iex\n\n' +
                  '方法2：\n' +
                  '  1. 访问 https://ollama.com/download\n' +
                  '  2. 下载 Windows 版本并安装\n' +
                  '  3. 安装完成后，在命令行运行：ollama pull qwen2.5:1.5b\n' +
                  '  4. 重启本应用即可使用';
      
      setMessage({ type: 'error', text: helpText });
    } finally {
      setInstalling({ ...installing, aiModel: false });
      setDownloadProgress({ ...downloadProgress, aiModel: 0 });
      setProgressStatus({ ...progressStatus, aiModel: '' });
    }
  };

  const handleUninstallAiModel = async () => {
    setConfirmDialog({
      open: true,
      title: '确认卸载 AI 模型',
      description: '卸载后将无法继续使用本地 AI 智能脱敏功能。',
      onConfirm: async () => {
        setConfirmDialog((prev) => ({ ...prev, open: false }));
        setUninstalling((prev) => ({ ...prev, aiModel: true }));
        setMessage(null);

        try {
          const result = await tauriCommands.uninstallAiModel();

          setMessage({ type: 'success', text: result });
          await checkServicesStatus();
        } catch (error) {
          console.error('Failed to uninstall AI model:', error);
          setMessage({ type: 'error', text: `卸载失败: ${error}` });
        } finally {
          setUninstalling((prev) => ({ ...prev, aiModel: false }));
        }
      },
    });
  };

  const handleSelectPath = async (service: 'ocr' | 'aiModel') => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: `选择 ${service === 'ocr' ? 'OCR' : 'AI 模型'} 安装目录`,
      });

      if (selected && typeof selected === 'string') {
        setCustomPaths({
          ...customPaths,
          [service]: selected,
        });
      }
    } catch (error) {
      console.error('Failed to select directory:', error);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <Loader2 className="w-8 h-8 animate-spin text-blue-600" />
      </div>
    );
  }

  return (
    <div className="p-6 max-w-4xl mx-auto">
      <div className="mb-6">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-bold text-gray-900 mb-2">增强服务</h2>
            <p className="text-gray-600">
              安装和管理应用的增强功能，提升文件处理能力
            </p>
            <p className="text-xs text-gray-500 mt-1">
              当前平台：{platformLabel}
            </p>
          </div>
          <div className="flex items-center space-x-3">
            <button
              onClick={handleStartOllama}
              disabled={startingOllama || loading}
              className="inline-flex items-center px-4 py-2 bg-green-600 text-white rounded-md hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {startingOllama ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  启动中...
                </>
              ) : (
                <>
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  启动 Ollama
                </>
              )}
            </button>
            <button
              onClick={handleScanServices}
              disabled={scanning || loading}
              className="inline-flex items-center px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {scanning ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  扫描中...
                </>
              ) : (
                <>
                  <svg className="w-4 h-4 mr-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                  </svg>
                  一键扫描
                </>
              )}
            </button>
          </div>
        </div>
      </div>

      {/* 消息提示 */}
      {message && (
        <div className={`mb-6 p-4 rounded-lg border ${
          message.type === 'success' ? 'bg-green-50 border-green-200' :
          message.type === 'error' ? 'bg-red-50 border-red-200' :
          'bg-blue-50 border-blue-200'
        }`}>
          <div className="flex items-start space-x-3">
            <div className="flex-shrink-0">
              {message.type === 'success' && (
                <CheckCircle className="w-5 h-5 text-green-600" />
              )}
              {message.type === 'error' && (
                <AlertCircle className="w-5 h-5 text-red-600" />
              )}
              {message.type === 'info' && (
                <Loader2 className="w-5 h-5 text-blue-600 animate-spin" />
              )}
            </div>
            <div className="flex-1">
              <p className={`text-sm ${
                message.type === 'success' ? 'text-green-800' :
                message.type === 'error' ? 'text-red-800' :
                'text-blue-800'
              }`}>
                {message.text}
              </p>
            </div>
            <button
              onClick={() => setMessage(null)}
              className="flex-shrink-0 text-gray-400 hover:text-gray-600"
            >
              <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
                <path fillRule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clipRule="evenodd" />
              </svg>
            </button>
          </div>
        </div>
      )}

      {/* 服务卡片网格 */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* OCR 服务卡片 */}
        <div className="bg-white border border-gray-200 rounded-lg shadow-sm overflow-hidden">
          <div className="p-6">
            <div className="flex items-start justify-between">
              <div className="flex items-start space-x-4">
                <div className="flex-shrink-0">
                  <div className="w-12 h-12 bg-blue-100 rounded-lg flex items-center justify-center">
                    <Package className="w-6 h-6 text-blue-600" />
                  </div>
                </div>
                <div className="flex-1">
                  <h3 className="text-lg font-semibold text-gray-900 mb-1">
                    OCR 文字识别服务
                  </h3>
                  <p className="text-sm text-gray-600 mb-3">
                    为 PDF 文本提取与后续脱敏提供本地运行时支持
                  </p>
                  
                  {/* 状态标签 */}
                  <div className="flex items-center space-x-2">
                    {serviceStatus.ocr ? (
                      <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">
                        <CheckCircle className="w-3 h-3 mr-1" />
                        已安装
                      </span>
                    ) : (
                      <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-800">
                        <AlertCircle className="w-3 h-3 mr-1" />
                        未安装
                      </span>
                    )}
                    <span className="text-xs text-gray-500">{isMac ? '模式: 本地 venv' : '模式: 嵌入式 Python'}</span>
                  </div>
                </div>
              </div>
            </div>

            {/* 功能特性 */}
            <div className="mt-4 pt-4 border-t border-gray-100">
              <h4 className="text-sm font-medium text-gray-900 mb-2">功能特性</h4>
              <ul className="space-y-1 text-sm text-gray-600">
                <li className="flex items-center">
                  <CheckCircle className="w-4 h-4 text-green-500 mr-2 flex-shrink-0" />
                  自动检测本地运行时是否可用
                </li>
                <li className="flex items-center">
                  <CheckCircle className="w-4 h-4 text-green-500 mr-2 flex-shrink-0" />
                  macOS 使用系统 Python 创建隔离环境
                </li>
                <li className="flex items-center">
                  <CheckCircle className="w-4 h-4 text-green-500 mr-2 flex-shrink-0" />
                  本地离线处理，保护隐私
                </li>
              </ul>
            </div>

            {/* 操作按钮 */}
            <div className="mt-6 flex items-center space-x-3">
              {!serviceStatus.ocr ? (
                <button
                  onClick={handleInstallOcr}
                  disabled={installing.ocr || serviceStatus.ocr}
                  className="inline-flex items-center px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed text-sm"
                >
                  {installing.ocr ? (
                    <>
                      <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                      安装中...
                    </>
                  ) : (
                    <>
                      <Download className="w-4 h-4 mr-2" />
                      一键安装
                    </>
                  )}
                </button>
              ) : (
                <button
                  onClick={handleUninstallOcr}
                  disabled={uninstalling.ocr}
                  className="inline-flex items-center px-4 py-2 border border-red-300 text-red-700 rounded-md hover:bg-red-50 disabled:opacity-50 disabled:cursor-not-allowed text-sm"
                >
                  {uninstalling.ocr ? (
                    <>
                      <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                      卸载中...
                    </>
                  ) : (
                    <>
                      <Trash2 className="w-4 h-4 mr-2" />
                      完全卸载
                    </>
                  )}
                </button>
              )}
            </div>

            {/* 下载进度 */}
            {installing.ocr && downloadProgress.ocr > 0 && (
              <div className="mt-4">
                <div className="flex items-center justify-between text-sm text-gray-600 mb-1">
                  <span className="truncate mr-2">{progressStatus.ocr || '下载进度'}</span>
                  <span className="font-medium">{downloadProgress.ocr.toFixed(1)}%</span>
                </div>
                <div className="w-full bg-gray-200 rounded-full h-2">
                  <div
                    className="bg-blue-600 h-2 rounded-full transition-all duration-300"
                    style={{ width: `${downloadProgress.ocr}%` }}
                  />
                </div>
              </div>
            )}
          </div>
        </div>

        {/* AI 脱敏模型卡片 */}
        <div className="bg-white border border-gray-200 rounded-lg shadow-sm overflow-hidden">
          <div className="p-6">
            <div className="flex items-start justify-between">
              <div className="flex items-start space-x-4">
                <div className="flex-shrink-0">
                  <div className="w-12 h-12 bg-purple-100 rounded-lg flex items-center justify-center">
                    <Brain className="w-6 h-6 text-purple-600" />
                  </div>
                </div>
                <div className="flex-1">
                  <h3 className="text-lg font-semibold text-gray-900 mb-1">
                    AI 智能脱敏模型
                  </h3>
                  <p className="text-sm text-gray-600 mb-3">
                    基于 Qwen2.5 的本地 AI 模型，智能识别敏感信息
                  </p>
                  
                  {/* 状态标签 */}
                  <div className="flex items-center space-x-2">
                    {serviceStatus.aiModel ? (
                      <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-800">
                        <CheckCircle className="w-3 h-3 mr-1" />
                        已安装
                      </span>
                    ) : (
                      <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-800">
                        <AlertCircle className="w-3 h-3 mr-1" />
                        未安装
                      </span>
                    )}
                    <span className="text-xs text-gray-500">模型: qwen2.5:1.5b</span>
                    <span className={`text-xs ${serviceStatus.ollamaRunning ? 'text-green-600' : 'text-amber-600'}`}>
                      {serviceStatus.ollamaInstalled
                        ? serviceStatus.ollamaRunning ? 'Ollama 服务已运行' : 'Ollama 已安装但未启动'
                        : 'Ollama 未安装'}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            {/* 功能特性 */}
            <div className="mt-4 pt-4 border-t border-gray-100">
              <h4 className="text-sm font-medium text-gray-900 mb-2">功能特性</h4>
              <ul className="space-y-1 text-sm text-gray-600">
                <li className="flex items-center">
                  <CheckCircle className="w-4 h-4 text-green-500 mr-2 flex-shrink-0" />
                  智能识别姓名、身份证等敏感信息
                </li>
                <li className="flex items-center">
                  <CheckCircle className="w-4 h-4 text-green-500 mr-2 flex-shrink-0" />
                  上下文理解，减少误判
                </li>
                <li className="flex items-center">
                  <CheckCircle className="w-4 h-4 text-green-500 mr-2 flex-shrink-0" />
                  本地运行，数据不出本地
                </li>
              </ul>
            </div>

            {/* 操作按钮 */}
            <div className="mt-6 flex items-center space-x-3 flex-wrap gap-y-2">
              {!serviceStatus.aiModel ? (
                <>
                  {serviceStatus.ollamaInstalled ? (
                    <button
                      onClick={handleInstallAiModel}
                      disabled={installing.aiModel}
                      className="inline-flex items-center px-4 py-2 bg-purple-600 text-white rounded-md hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed text-sm"
                    >
                      {installing.aiModel ? (
                        <>
                          <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                          安装中...
                        </>
                      ) : (
                        <>
                          <Download className="w-4 h-4 mr-2" />
                          安装 AI 模型
                        </>
                      )}
                    </button>
                  ) : (
                    <>
                      <a
                        href="https://ollama.com/download"
                        target="_blank"
                        rel="noopener noreferrer"
                        className="inline-flex items-center px-4 py-2 bg-purple-600 text-white rounded-md hover:bg-purple-700 text-sm"
                      >
                        <ExternalLink className="w-4 h-4 mr-2" />
                        下载 Ollama
                      </a>
                      <button
                        onClick={handleInstallAiModel}
                        disabled={installing.aiModel}
                        className="inline-flex items-center px-4 py-2 border border-purple-300 text-purple-700 rounded-md hover:bg-purple-50 disabled:opacity-50 disabled:cursor-not-allowed text-sm"
                      >
                        {installing.aiModel ? (
                          <>
                            <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                            检查中...
                          </>
                        ) : (
                          <>
                            <Download className="w-4 h-4 mr-2" />
                            安装 AI 模型
                          </>
                        )}
                      </button>
                    </>
                  )}
                </>
              ) : (
                <button
                  onClick={handleUninstallAiModel}
                  disabled={uninstalling.aiModel}
                  className="inline-flex items-center px-4 py-2 border border-red-300 text-red-700 rounded-md hover:bg-red-50 disabled:opacity-50 disabled:cursor-not-allowed text-sm"
                >
                  {uninstalling.aiModel ? (
                    <>
                      <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                      卸载中...
                    </>
                  ) : (
                    <>
                      <Trash2 className="w-4 h-4 mr-2" />
                      完全卸载
                    </>
                  )}
                </button>
              )}
              <a
                href="https://ollama.com/library/qwen2.5:1.5b"
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center px-4 py-2 border border-gray-300 text-gray-700 rounded-md hover:bg-gray-50 text-sm"
              >
                <ExternalLink className="w-4 h-4 mr-2" />
                了解更多
              </a>
            </div>

            {/* 下载进度 */}
            {installing.aiModel && downloadProgress.aiModel > 0 && (
              <div className="mt-4">
                <div className="flex items-center justify-between text-sm text-gray-600 mb-1">
                  <span className="truncate mr-2">{progressStatus.aiModel || '下载进度'}</span>
                  <span className="font-medium">{downloadProgress.aiModel.toFixed(1)}%</span>
                </div>
                <div className="w-full bg-gray-200 rounded-full h-2">
                  <div
                    className="bg-purple-600 h-2 rounded-full transition-all duration-300"
                    style={{ width: `${downloadProgress.aiModel}%` }}
                  />
                </div>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* 帮助信息 */}
      <div className="mt-6 grid grid-cols-1 lg:grid-cols-2 gap-4">
        <div className="p-4 bg-blue-50 border border-blue-200 rounded-lg">
          <h3 className="font-medium text-blue-900 mb-2">💡 OCR 服务说明</h3>
          <ul className="text-sm text-blue-800 space-y-1 list-disc list-inside">
            <li>Windows 使用嵌入式 Python，macOS 使用系统 Python venv</li>
            <li>当前轻量运行时优先支持 PDF 文本提取</li>
            <li>所有处理均在本地完成</li>
          </ul>
        </div>

        <div className="p-4 bg-purple-50 border border-purple-200 rounded-lg">
          <h3 className="font-medium text-purple-900 mb-2">🤖 AI 模型说明</h3>
          <ul className="text-sm text-purple-800 space-y-1 list-disc list-inside">
            <li>使用 Qwen2.5:1.5b 轻量级模型（约 1GB）</li>
            <li>自动检测系统已安装的 Ollama，无需重复安装</li>
            <li>macOS 优先唤起 Ollama.app，避免重复前台 `serve` 进程</li>
            <li>提供智能敏感信息识别能力</li>
          </ul>
        </div>
      </div>

      {/* 路径选择对话框 */}
      <Dialog open={showPathDialog !== null} onOpenChange={(open) => !open && setShowPathDialog(null)}>
        <DialogContent className="sm:max-w-[500px]">
          <DialogHeader>
            <DialogTitle>
              选择安装路径 - {showPathDialog === 'ocr' ? 'OCR 服务' : 'AI 模型'}
            </DialogTitle>
            <DialogDescription>
              请选择 {showPathDialog === 'ocr' ? 'OCR 服务' : 'AI 模型'} 的安装目录，建议选择当前用户可写目录，避免系统保护目录
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <div className="space-y-2">
              <Label htmlFor="install-path">安装路径</Label>
              <div className="flex gap-2">
                <Input
                  id="install-path"
                  value={showPathDialog ? customPaths[showPathDialog] : ''}
                  placeholder={`默认路径：${showPathDialog ? defaultInstallPlaceholder(showPathDialog) : ''}`}
                  readOnly
                  className="flex-1"
                />
                <Button
                  type="button"
                  variant="outline"
                  size="icon"
                  onClick={() => showPathDialog && handleSelectPath(showPathDialog)}
                >
                  <FolderOpen className="h-4 w-4" />
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">
                {showPathDialog && customPaths[showPathDialog] 
                  ? '已选择自定义路径' 
                  : '默认安装到应用数据目录'}
              </p>
            </div>

            <div className="flex items-start gap-2 text-sm text-muted-foreground bg-blue-50 p-3 rounded-md">
              <AlertCircle className="h-4 w-4 mt-0.5 flex-shrink-0 text-blue-600" />
              <div>
                <p className="font-medium text-blue-900 mb-1">安装说明：</p>
                <ul className="list-disc list-inside space-y-1 text-xs text-blue-800">
                  {showPathDialog === 'ocr' ? (
                    <>
                      <li>{isMac ? '将基于系统 Python 创建 venv 并安装 PyMuPDF' : '将下载 Python 运行时并安装 PyMuPDF'}</li>
                      <li>建议选择可写目录，避免系统保护目录</li>
                      <li>当前轻量运行时仅支持 PDF 文本提取</li>
                    </>
                  ) : (
                    <>
                      <li>{isMac ? '建议先完成 Ollama.app 安装并确认服务可启动' : '将检查或准备 Ollama 与 AI 模型（约 1GB）'}</li>
                      <li>建议选择可写目录，避免系统保护目录</li>
                      <li>若 Ollama 已安装但服务未启动，需先启动后再安装模型</li>
                    </>
                  )}
                </ul>
              </div>
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={() => setShowPathDialog(null)}>
              取消
            </Button>
            <Button 
              onClick={() => {
                if (showPathDialog === 'ocr') {
                  handleConfirmInstallOcr();
                } else {
                  handleConfirmInstallAiModel();
                }
              }}
            >
              <Download className="h-4 w-4 mr-2" />
              开始安装
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={confirmDialog.open} onOpenChange={(open) => !open && setConfirmDialog((prev) => ({ ...prev, open: false }))}>
        <DialogContent className="sm:max-w-[420px]">
          <DialogHeader>
            <DialogTitle>{confirmDialog.title}</DialogTitle>
            <DialogDescription>{confirmDialog.description}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setConfirmDialog((prev) => ({ ...prev, open: false }))}>
              取消
            </Button>
            <Button variant="destructive" onClick={() => void confirmDialog.onConfirm()}>
              确认
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
