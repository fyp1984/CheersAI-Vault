import { useState, useEffect } from 'react';
import { Download, CheckCircle, AlertCircle, Loader2, Trash2, Terminal } from 'lucide-react';
import { tauriCommands } from '@/lib/tauri';
import { listen } from '@tauri-apps/api/event';
import type { InstallerProgress } from '@/types/commands';

export function InstallerTest() {
  const [pythonAvailable, setPythonAvailable] = useState(false);
  const [ocrInstalled, setOcrInstalled] = useState(false);
  const [ollamaInstalled, setOllamaInstalled] = useState(false);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState({
    ocr: false,
    ollama: false,
  });
  const [uninstalling, setUninstalling] = useState({
    ocr: false,
    ollama: false,
  });
  const [progress, setProgress] = useState<{
    ocr: InstallerProgress | null;
    ollama: InstallerProgress | null;
  }>({
    ocr: null,
    ollama: null,
  });
  const [logs, setLogs] = useState<string[]>([]);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);

  useEffect(() => {
    checkStatus();
    setupEventListeners();
  }, []);

  const setupEventListeners = () => {
    // 监听 OCR 安装进度
    listen<InstallerProgress>('ocr-install-progress', (event) => {
      setProgress(prev => ({ ...prev, ocr: event.payload }));
      addLog(`[OCR] ${event.payload.log}`);
    });

    // 监听 OCR 卸载进度
    listen<InstallerProgress>('ocr-uninstall-progress', (event) => {
      setProgress(prev => ({ ...prev, ocr: event.payload }));
      addLog(`[OCR] ${event.payload.log}`);
    });

    // 监听 Ollama 安装进度
    listen<InstallerProgress>('ollama-install-progress', (event) => {
      setProgress(prev => ({ ...prev, ollama: event.payload }));
      addLog(`[Ollama] ${event.payload.log}`);
    });

    // 监听 Ollama 卸载进度
    listen<InstallerProgress>('ollama-uninstall-progress', (event) => {
      setProgress(prev => ({ ...prev, ollama: event.payload }));
      addLog(`[Ollama] ${event.payload.log}`);
    });
  };

  const addLog = (message: string) => {
    setLogs(prev => [...prev.slice(-100), message]); // 保留最后 100 条日志
  };

  const checkStatus = async () => {
    try {
      setLoading(true);
      
      const [python, ocr, ollama] = await Promise.all([
        tauriCommands.checkPythonAvailable().catch(() => false),
        tauriCommands.checkOcrInstalled().catch(() => false),
        tauriCommands.checkOllamaInstalled().catch(() => false),
      ]);
      
      setPythonAvailable(python);
      setOcrInstalled(ocr);
      setOllamaInstalled(ollama);
      
      if (!python) {
        setMessage({
          type: 'error',
          text: 'Python 未安装！请先安装 Python 3.7+ 才能使用脚本安装功能。'
        });
      }
    } finally {
      setLoading(false);
    }
  };

  const handleInstallOcr = async () => {
    if (!pythonAvailable) {
      setMessage({ type: 'error', text: '请先安装 Python！' });
      return;
    }

    try {
      setInstalling({ ...installing, ocr: true });
      setMessage(null);
      setProgress({ ...progress, ocr: null });
      setLogs([]);
      addLog('开始安装 OCR 环境...');

      await tauriCommands.installOcrWithScript();

      setMessage({ type: 'success', text: 'OCR 环境安装成功！' });
      await checkStatus();
    } catch (error) {
      console.error('Failed to install OCR:', error);
      setMessage({ type: 'error', text: `安装失败: ${error}` });
      addLog(`错误: ${error}`);
    } finally {
      setInstalling({ ...installing, ocr: false });
    }
  };

  const handleUninstallOcr = async () => {
    if (!confirm('确定要卸载 OCR 环境吗？')) {
      return;
    }

    try {
      setUninstalling({ ...uninstalling, ocr: true });
      setMessage(null);
      setProgress({ ...progress, ocr: null });
      setLogs([]);
      addLog('开始卸载 OCR 环境...');

      await tauriCommands.uninstallOcrWithScript();

      setMessage({ type: 'success', text: 'OCR 环境已卸载' });
      await checkStatus();
    } catch (error) {
      console.error('Failed to uninstall OCR:', error);
      setMessage({ type: 'error', text: `卸载失败: ${error}` });
      addLog(`错误: ${error}`);
    } finally {
      setUninstalling({ ...uninstalling, ocr: false });
    }
  };

  const handleInstallOllama = async () => {
    if (!pythonAvailable) {
      setMessage({ type: 'error', text: '请先安装 Python！' });
      return;
    }

    try {
      setInstalling({ ...installing, ollama: true });
      setMessage(null);
      setProgress({ ...progress, ollama: null });
      setLogs([]);
      addLog('开始安装 Ollama + AI 模型...');

      await tauriCommands.installOllamaWithScript();

      setMessage({ type: 'success', text: 'Ollama + AI 模型安装成功！' });
      await checkStatus();
    } catch (error) {
      console.error('Failed to install Ollama:', error);
      setMessage({ type: 'error', text: `安装失败: ${error}` });
      addLog(`错误: ${error}`);
    } finally {
      setInstalling({ ...installing, ollama: false });
    }
  };

  const handleUninstallOllama = async () => {
    if (!confirm('确定要完全卸载 Ollama 吗？这将删除所有程序文件和用户数据。')) {
      return;
    }

    try {
      setUninstalling({ ...uninstalling, ollama: true });
      setMessage(null);
      setProgress({ ...progress, ollama: null });
      setLogs([]);
      addLog('开始卸载 Ollama...');

      await tauriCommands.uninstallOllamaWithScript();

      setMessage({ type: 'success', text: 'Ollama 已完全卸载' });
      await checkStatus();
    } catch (error) {
      console.error('Failed to uninstall Ollama:', error);
      setMessage({ type: 'error', text: `卸载失败: ${error}` });
      addLog(`错误: ${error}`);
    } finally {
      setUninstalling({ ...uninstalling, ollama: false });
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-screen">
        <Loader2 className="w-8 h-8 animate-spin text-primary" />
      </div>
    );
  }

  return (
    <div className="container mx-auto p-6 max-w-6xl">
      <div className="mb-6">
        <h1 className="text-3xl font-bold mb-2">安装器测试</h1>
        <p className="text-muted-foreground">
          使用独立 Python 脚本安装 OCR 和 Ollama 服务
        </p>
      </div>

      {/* Python 状态 */}
      <div className="mb-6 p-4 border rounded-lg">
        <div className="flex items-center gap-2">
          {pythonAvailable ? (
            <>
              <CheckCircle className="w-5 h-5 text-green-500" />
              <span className="font-medium">Python 已安装</span>
            </>
          ) : (
            <>
              <AlertCircle className="w-5 h-5 text-red-500" />
              <span className="font-medium">Python 未安装</span>
              <span className="text-sm text-muted-foreground ml-2">
                请先安装 Python 3.7+ 才能使用脚本安装功能
              </span>
            </>
          )}
        </div>
      </div>

      {/* 消息提示 */}
      {message && (
        <div
          className={`mb-6 p-4 border rounded-lg ${
            message.type === 'success'
              ? 'bg-green-50 border-green-200 text-green-800'
              : message.type === 'error'
              ? 'bg-red-50 border-red-200 text-red-800'
              : 'bg-blue-50 border-blue-200 text-blue-800'
          }`}
        >
          <div className="flex items-start gap-2">
            {message.type === 'success' ? (
              <CheckCircle className="w-5 h-5 flex-shrink-0 mt-0.5" />
            ) : message.type === 'error' ? (
              <AlertCircle className="w-5 h-5 flex-shrink-0 mt-0.5" />
            ) : (
              <Terminal className="w-5 h-5 flex-shrink-0 mt-0.5" />
            )}
            <p className="whitespace-pre-wrap">{message.text}</p>
          </div>
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mb-6">
        {/* OCR 服务 */}
        <div className="border rounded-lg p-6">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h2 className="text-xl font-semibold">OCR 服务</h2>
              <p className="text-sm text-muted-foreground">
                Python + PyMuPDF + EasyOCR (~530MB)
              </p>
            </div>
            {ocrInstalled ? (
              <CheckCircle className="w-6 h-6 text-green-500" />
            ) : (
              <AlertCircle className="w-6 h-6 text-gray-400" />
            )}
          </div>

          {progress.ocr && (
            <div className="mb-4">
              <div className="flex justify-between text-sm mb-1">
                <span>{progress.ocr.status}</span>
                <span>{progress.ocr.percentage.toFixed(1)}%</span>
              </div>
              <div className="w-full bg-gray-200 rounded-full h-2">
                <div
                  className="bg-primary h-2 rounded-full transition-all"
                  style={{ width: `${progress.ocr.percentage}%` }}
                />
              </div>
            </div>
          )}

          <div className="flex gap-2">
            {!ocrInstalled ? (
              <button
                onClick={handleInstallOcr}
                disabled={installing.ocr || !pythonAvailable}
                className="flex-1 flex items-center justify-center gap-2 px-4 py-2 bg-primary text-primary-foreground rounded-md hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {installing.ocr ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    安装中...
                  </>
                ) : (
                  <>
                    <Download className="w-4 h-4" />
                    安装
                  </>
                )}
              </button>
            ) : (
              <button
                onClick={handleUninstallOcr}
                disabled={uninstalling.ocr}
                className="flex-1 flex items-center justify-center gap-2 px-4 py-2 bg-destructive text-destructive-foreground rounded-md hover:bg-destructive/90 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {uninstalling.ocr ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    卸载中...
                  </>
                ) : (
                  <>
                    <Trash2 className="w-4 h-4" />
                    卸载
                  </>
                )}
              </button>
            )}
          </div>
        </div>

        {/* Ollama + AI 模型 */}
        <div className="border rounded-lg p-6">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h2 className="text-xl font-semibold">Ollama + AI 模型</h2>
              <p className="text-sm text-muted-foreground">
                Ollama + qwen2.5:1.5b (~1.6GB)
              </p>
            </div>
            {ollamaInstalled ? (
              <CheckCircle className="w-6 h-6 text-green-500" />
            ) : (
              <AlertCircle className="w-6 h-6 text-gray-400" />
            )}
          </div>

          {progress.ollama && (
            <div className="mb-4">
              <div className="flex justify-between text-sm mb-1">
                <span>{progress.ollama.status}</span>
                <span>{progress.ollama.percentage.toFixed(1)}%</span>
              </div>
              <div className="w-full bg-gray-200 rounded-full h-2">
                <div
                  className="bg-primary h-2 rounded-full transition-all"
                  style={{ width: `${progress.ollama.percentage}%` }}
                />
              </div>
            </div>
          )}

          <div className="flex gap-2">
            {!ollamaInstalled ? (
              <button
                onClick={handleInstallOllama}
                disabled={installing.ollama || !pythonAvailable}
                className="flex-1 flex items-center justify-center gap-2 px-4 py-2 bg-primary text-primary-foreground rounded-md hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {installing.ollama ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    安装中...
                  </>
                ) : (
                  <>
                    <Download className="w-4 h-4" />
                    安装
                  </>
                )}
              </button>
            ) : (
              <button
                onClick={handleUninstallOllama}
                disabled={uninstalling.ollama}
                className="flex-1 flex items-center justify-center gap-2 px-4 py-2 bg-destructive text-destructive-foreground rounded-md hover:bg-destructive/90 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {uninstalling.ollama ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    卸载中...
                  </>
                ) : (
                  <>
                    <Trash2 className="w-4 h-4" />
                    卸载
                  </>
                )}
              </button>
            )}
          </div>
        </div>
      </div>

      {/* 日志输出 */}
      {logs.length > 0 && (
        <div className="border rounded-lg p-4">
          <h3 className="text-lg font-semibold mb-2 flex items-center gap-2">
            <Terminal className="w-5 h-5" />
            安装日志
          </h3>
          <div className="bg-black text-green-400 p-4 rounded font-mono text-sm h-64 overflow-y-auto">
            {logs.map((log, index) => (
              <div key={index}>{log}</div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
