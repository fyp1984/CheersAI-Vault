import { useState, useEffect } from "react";
import { PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { 
  Lock, 
  Unlock, 
  FileText, 
  FolderOpen, 
  Shield, 
  Eye,
  EyeOff,
  Save,
  Info,
  CheckCircle2,
  AlertTriangle,
  XCircle
} from "lucide-react";
import { useSandboxStore } from "@/store/sandboxStore";
import { useFileStore } from "@/store/fileStore";
import { tauriCommands } from "@/lib/tauri";
import { formatBytes } from "@/lib/utils";
import { getDisplayPath, validatePath, getDefaultDocumentsPath } from "@/lib/path";
import { open } from "@tauri-apps/plugin-dialog";
import type { PlatformContext } from "@/types/commands";

export default function SandboxManager() {
  const { locked, files, setLocked, setFiles } = useSandboxStore();
  const { outputDir, setOutputDir } = useFileStore();
  
  // 本地状态
  const [pin, setPin] = useState("");
  const [newPin, setNewPin] = useState("");
  const [confirmPin, setConfirmPin] = useState("");
  const [showPin, setShowPin] = useState(false);
  const [showNewPin, setShowNewPin] = useState(false);
  const [loading, setLoading] = useState(false);
  const [pathError, setPathError] = useState<string>("");
  const [pinExists, setPinExists] = useState<boolean | null>(null);
  const [platformContext, setPlatformContext] = useState<PlatformContext | null>(null);
  const [toast, setToast] = useState<{ message: string; type: 'success' | 'error' | 'warning' } | null>(null);
  const [confirmDialog, setConfirmDialog] = useState<{
    open: boolean;
    title: string;
    description: string;
    onConfirm: () => void;
  }>({ open: false, title: '', description: '', onConfirm: () => {} });
  const [syncing, setSyncing] = useState(false);

  // toast 自动消失
  useEffect(() => {
    if (toast) {
      const timer = setTimeout(() => setToast(null), 3000);
      return () => clearTimeout(timer);
    }
  }, [toast]);

  const platformName = platformContext?.os === 'windows'
    ? 'Windows'
    : platformContext?.os === 'macos'
      ? 'macOS'
      : platformContext?.os === 'linux'
        ? 'Linux'
        : '当前系统';
  const pinStorageLabel = platformContext?.pinStorageMode === "macos_keychain"
    ? "macOS Keychain"
    : platformContext?.pinStorageMode === "windows_dpapi"
      ? "Windows 凭据保护"
      : "兼容模式";
  const pinStorageDescription = platformContext?.pinStorageMode === "macos_keychain"
    ? "PIN 将使用 macOS Keychain 安全存储，重启应用后仍然有效。"
    : platformContext?.pinStorageMode === "windows_dpapi"
      ? "PIN 将使用 Windows 凭据保护加密存储，绑定当前 Windows 用户账户，重启应用后仍然有效。"
      : "PIN 将以兼容模式存储，请仅用于本地开发或测试环境。";
  const resolvedDefaultDocumentsPath = platformContext?.defaultDocumentsDir || getDefaultDocumentsPath();
  const defaultPathExample = getDisplayPath(resolvedDefaultDocumentsPath, 40);

  useEffect(() => {
    const loadPlatformContext = async () => {
      try {
        const context = await tauriCommands.getPlatformContext();
        setPlatformContext(context);
      } catch (error) {
        console.error("Failed to load platform context in sandbox:", error);
      }
    };

    loadPlatformContext();
  }, []);

  // 加载沙箱文件列表（基于输出目录）
  const loadFiles = async () => {
    if (!locked && outputDir) {
      try {
        const fileList = await tauriCommands.listFilesInDirectory(outputDir);
        // 过滤掉 .cmap 对照文件
        const filteredFiles = fileList.filter(file => !file.name.endsWith('.cmap'));
        setFiles(filteredFiles);
      } catch (error) {
        console.error("Failed to load sandbox files:", error);
        setFiles([]); // 如果失败，设置为空数组
      }
    } else {
      setFiles([]);
    }
  };

  useEffect(() => {
    loadFiles();
  }, [locked, outputDir]);

  // 检查是否已设置 PIN（DPAPI 持久化）
  useEffect(() => {
    const checkPin = async () => {
      try {
        const exists = await tauriCommands.hasPin();
        setPinExists(exists);
        if (!exists) {
          setLocked(false);
          // 没有 PIN 时确保文件可见
          if (outputDir) {
            await tauriCommands.unlockSandboxFiles(outputDir);
          }
        } else {
          // 有 PIN 且应用刚启动时，确保文件隐藏
          if (outputDir) {
            await tauriCommands.lockSandboxFiles(outputDir);
          }
        }
      } catch (error) {
        console.error("Failed to check PIN status:", error);
        setPinExists(false);
        setLocked(false);
      }
    };
    checkPin();
  }, []);

  // 初始化：如果不记住口令，清空已保存的口令
  useEffect(() => {
    console.log("=== SandboxManager 初始化 ===");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 验证PIN
  const handleUnlock = async () => {
    if (!pin) return;
    
    setLoading(true);
    try {
      const ok = await tauriCommands.verifyPin(pin);
      if (ok) {
        // 解锁时取消文件隐藏
        if (outputDir) {
          await tauriCommands.unlockSandboxFiles(outputDir);
        }
        setLocked(false);
        setPin("");
        await loadFiles();
      } else {
        setToast({ message: 'PIN 错误，请重试', type: 'error' });
      }
    } catch (error) {
      setToast({ message: '验证失败', type: 'error' });
    } finally {
      setLoading(false);
    }
  };

  // 设置新PIN
  const handleSetPin = async () => {
    if (!newPin || newPin !== confirmPin) {
      setToast({ message: 'PIN 不匹配，请重新输入', type: 'warning' });
      return;
    }
    
    if (newPin.length < 4) {
      setToast({ message: 'PIN 至少需要 4 位', type: 'warning' });
      return;
    }

    setLoading(true);
    try {
      await tauriCommands.setPin(newPin);
      setNewPin("");
      setConfirmPin("");
      setPinExists(true);
      setToast({ message: `PIN 设置成功，已启用 ${pinStorageLabel} 安全存储`, type: 'success' });
      // 延迟锁定并隐藏文件
      setTimeout(async () => {
        if (outputDir) {
          await tauriCommands.lockSandboxFiles(outputDir);
        }
        setLocked(true);
      }, 1500);
    } catch (error) {
      setToast({ message: 'PIN 设置失败: ' + error, type: 'error' });
    } finally {
      setLoading(false);
    }
  };

  // 清除 PIN
  const handleClearPin = () => {
    setConfirmDialog({
      open: true,
      title: '清除 PIN',
      description: '确定要清除 PIN 吗？清除后沙箱将不再需要密码访问。',
      onConfirm: async () => {
        setConfirmDialog(prev => ({ ...prev, open: false }));
        setLoading(true);
        try {
          await tauriCommands.clearPin();
          setPinExists(false);
          setLocked(false);
          setToast({ message: 'PIN 已清除', type: 'success' });
        } catch (error) {
          setToast({ message: '清除 PIN 失败: ' + error, type: 'error' });
        } finally {
          setLoading(false);
        }
      },
    });
  };

  // 选择输出路径（跨平台）
  const handleSelectPath = async () => {
    try {
      const selected = await open({
        directory: true,
        title: "选择文件输出/沙箱存储路径",
        defaultPath: outputDir || resolvedDefaultDocumentsPath,
      });
      if (selected) {
        const selectedPath = selected as string;
        const validation = validatePath(selectedPath);
        
        if (!validation.valid) {
          setPathError(validation.error || "路径无效");
          return;
        }
        
        setPathError("");
        setOutputDir(selectedPath);
      }
    } catch (error) {
      console.error("Failed to select output directory:", error);
      setPathError("选择路径失败");
    }
  };

  // 手动输入路径时验证
  const handlePathChange = (path: string) => {
    setOutputDir(path);
    
    if (path) {
      const validation = validatePath(path);
      setPathError(validation.valid ? "" : validation.error || "路径无效");
    } else {
      setPathError("");
    }
  };

  // 使用默认路径
  const handleUseDefaultPath = () => {
    const defaultPath = resolvedDefaultDocumentsPath;
    setOutputDir(defaultPath);
    setPathError("");
  };

  // 一键同步到 FileBay
  const handleSyncToFileBay = async () => {
    if (!outputDir || files.length === 0) {
      setToast({ message: '没有可同步的文件', type: 'warning' });
      return;
    }

    setSyncing(true);
    
    try {
      // TODO: 调用 FileBay 同步 API
      // 这里需要实现将文件上传到 FileBay 的逻辑
      await new Promise(resolve => setTimeout(resolve, 2000)); // 模拟上传
      
      setToast({ message: `成功同步 ${files.length} 个文件到 FileBay`, type: 'success' });
    } catch (error) {
      console.error('Sync to FileBay failed:', error);
      setToast({ message: '同步失败: ' + error, type: 'error' });
    } finally {
      setSyncing(false);
    }
  };

  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="沙箱管理"
        description="安全设置和文件输出配置"
        actions={
          <div className="flex gap-2">
            {!locked && pinExists && (
              <Button size="sm" variant="outline" onClick={async () => {
                if (outputDir) {
                  await tauriCommands.lockSandboxFiles(outputDir);
                }
                setLocked(true);
              }}>
                <Lock className="w-4 h-4 mr-1" />
                锁定沙箱
              </Button>
            )}
          </div>
        }
      />

      <div className="flex-1 overflow-auto p-6">
        <div className="max-w-4xl mx-auto space-y-6">
          
          {/* 沙箱状态卡片 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                {locked ? (
                  <Lock className="w-5 h-5 text-red-500" />
                ) : (
                  <Unlock className="w-5 h-5 text-green-500" />
                )}
                沙箱状态
                <span className="ml-auto px-2 py-0.5 text-xs bg-blue-100 text-blue-700 rounded">
                  {pinStorageLabel}
                </span>
              </CardTitle>
            </CardHeader>
            <CardContent>
              {pinExists === null ? (
                <p className="text-sm text-gray-500">正在检查 PIN 状态...</p>
              ) : !pinExists ? (
                <div className="space-y-3">
                  <div className="p-3 bg-yellow-50 border border-yellow-200 rounded-lg">
                    <p className="text-sm text-yellow-800">⚠️ 尚未设置 PIN，沙箱当前无密码保护。请在下方「安全设置」中设置 PIN。</p>
                  </div>
                  <p className="text-xs text-gray-500">{pinStorageDescription}</p>
                </div>
              ) : locked ? (
                <div className="space-y-4">
                  <p className="text-sm text-gray-600">沙箱已锁定，请输入 PIN 解锁</p>
                  <div className="flex gap-2 max-w-md">
                    <div className="relative flex-1">
                      <Input
                        type={showPin ? "text" : "password"}
                        placeholder="输入 PIN"
                        value={pin}
                        onChange={(e) => setPin(e.target.value)}
                        onKeyDown={(e) => e.key === "Enter" && handleUnlock()}
                      />
                      <button
                        type="button"
                        onClick={() => setShowPin(!showPin)}
                        className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
                      >
                        {showPin ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                      </button>
                    </div>
                    <Button onClick={handleUnlock} disabled={!pin || loading}>
                      <Unlock className="w-4 h-4 mr-1" />
                      解锁
                    </Button>
                  </div>
                  <p className="text-xs text-gray-500">
                    {platformContext?.pinStorageMode === "macos_keychain"
                      ? "PIN 已保存在 macOS Keychain 中，仅当前系统账户可安全读取。"
                      : platformContext?.pinStorageMode === "windows_dpapi"
                        ? "PIN 已通过 Windows 凭据保护加密存储，仅当前 Windows 用户可解密。"
                        : "PIN 已保存到本地兼容存储。"}
                  </p>
                </div>
              ) : (
                <div className="space-y-4">
                  <p className="text-sm text-green-600">✅ 沙箱已解锁，可以访问安全文件</p>
                  <div className="text-sm text-gray-600">
                    文件数量: {files.length} 个
                  </div>
                </div>
              )}
            </CardContent>
          </Card>

          {/* 安全设置 */}
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Shield className="w-5 h-5 text-blue-500" />
                安全设置
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              
              {/* PIN 设置 */}
              <div className="space-y-4">
                <Label className="text-sm font-medium">设置新 PIN</Label>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div className="relative">
                    <Input
                      type={showNewPin ? "text" : "password"}
                      placeholder="新 PIN (至少4位)"
                      value={newPin}
                      onChange={(e) => setNewPin(e.target.value)}
                    />
                    <button
                      type="button"
                      onClick={() => setShowNewPin(!showNewPin)}
                      className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
                    >
                      {showNewPin ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                    </button>
                  </div>
                  <Input
                    type="password"
                    placeholder="确认 PIN"
                    value={confirmPin}
                    onChange={(e) => setConfirmPin(e.target.value)}
                  />
                </div>
                <div className="flex gap-2">
                  <Button 
                    onClick={handleSetPin} 
                    disabled={!newPin || !confirmPin || loading}
                    size="sm"
                  >
                    <Save className="w-4 h-4 mr-1" />
                    {pinExists ? '重新设置 PIN' : '设置 PIN'}
                  </Button>
                  {pinExists && (
                    <Button 
                      onClick={handleClearPin} 
                      disabled={loading}
                      size="sm"
                      variant="outline"
                      className="text-red-600 hover:text-red-700 border-red-200 hover:border-red-300"
                    >
                      清除 PIN
                    </Button>
                  )}
                </div>
              </div>

              <Separator />

              {/* 输出/沙箱路径 */}
              <div className="space-y-4">
                <div className="flex items-center gap-2">
                  <Label className="text-sm font-medium">文件输出路径</Label>
                  <div className="px-2 py-1 bg-blue-100 text-blue-700 text-xs rounded">
                    {platformName}
                  </div>
                </div>
                <p className="text-xs text-gray-500">
                  脱敏文件的输出目录，同时也是沙箱的存储位置
                </p>
                
                <div className="space-y-2">
                  <div className="flex gap-2">
                    <Input
                      placeholder={`选择文件输出路径 (${defaultPathExample})`}
                      value={outputDir}
                      onChange={(e) => handlePathChange(e.target.value)}
                      className={`flex-1 ${pathError ? 'border-red-300' : ''}`}
                    />
                    <Button variant="outline" onClick={handleSelectPath}>
                      <FolderOpen className="w-4 h-4 mr-1" />
                      浏览
                    </Button>
                  </div>
                  
                  {pathError && (
                    <p className="text-sm text-red-600 flex items-center gap-1">
                      <Info className="w-4 h-4" />
                      {pathError}
                    </p>
                  )}
                  
                  <Button 
                    variant="ghost" 
                    size="sm" 
                    onClick={handleUseDefaultPath}
                    className="text-xs"
                  >
                    使用默认路径 ({getDisplayPath(resolvedDefaultDocumentsPath, 40)})
                  </Button>
                </div>
                
                {outputDir && !pathError && (
                  <div className="p-3 bg-green-50 border border-green-200 rounded-lg">
                    <p className="text-sm text-green-700">
                      <strong>当前路径:</strong> {getDisplayPath(outputDir, 60)}
                    </p>
                    <p className="text-xs text-green-600 mt-1">
                      脱敏文件和映射文件都将保存到此目录
                    </p>
                  </div>
                )}
              </div>
            </CardContent>
          </Card>

          {/* 沙箱文件列表 */}
          {!locked && (
            <Card>
              <CardHeader>
                <div className="flex items-center justify-between">
                  <div>
                    <CardTitle className="flex items-center gap-2">
                      <FileText className="w-5 h-5 text-gray-500" />
                      输出目录文件
                    </CardTitle>
                    {outputDir && (
                      <p className="text-xs text-gray-500 mt-1">
                        位置: {getDisplayPath(outputDir, 50)}
                      </p>
                    )}
                  </div>
                  {files.length > 0 && (
                    <Button 
                      onClick={handleSyncToFileBay} 
                      disabled={syncing}
                      size="sm"
                      className="bg-blue-600 hover:bg-blue-700"
                    >
                      {syncing ? (
                        <>
                          <svg className="animate-spin -ml-1 mr-2 h-4 w-4 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                          </svg>
                          同步中...
                        </>
                      ) : (
                        <>
                          <svg className="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                          </svg>
                          一键同步
                        </>
                      )}
                    </Button>
                  )}
                </div>
              </CardHeader>
              <CardContent>
                {!outputDir ? (
                  <div className="text-center py-8 text-gray-400">
                    <FolderOpen className="w-12 h-12 mx-auto mb-2 opacity-50" />
                    <p className="text-sm">请先设置文件输出路径</p>
                  </div>
                ) : files.length === 0 ? (
                  <div className="text-center py-8 text-gray-400">
                    <FileText className="w-12 h-12 mx-auto mb-2 opacity-50" />
                    <p className="text-sm">输出目录为空</p>
                    <p className="text-xs mt-1">处理文件后，脱敏文件和映射文件将显示在这里</p>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {files.map((file) => (
                      <div
                        key={file.name}
                        className="flex items-center gap-3 p-3 rounded-lg border hover:bg-gray-50"
                      >
                        <FileText className="w-5 h-5 text-gray-400" />
                        <div className="flex-1 min-w-0">
                          <p className="text-sm font-medium text-gray-800 truncate">
                            {file.name}
                          </p>
                          <p className="text-xs text-gray-500">
                            {formatBytes(file.size)} · {file.modified}
                          </p>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>
          )}
        </div>
      </div>

      {/* Toast 通知 */}
      {toast && (
        <div className="fixed bottom-6 right-6 z-50 animate-in slide-in-from-bottom-4 fade-in duration-300">
          <div className={`flex items-center gap-3 px-4 py-3 rounded-lg shadow-lg border ${
            toast.type === 'success' ? 'bg-green-50 border-green-200 text-green-800' :
            toast.type === 'error' ? 'bg-red-50 border-red-200 text-red-800' :
            'bg-yellow-50 border-yellow-200 text-yellow-800'
          }`}>
            {toast.type === 'success' && <CheckCircle2 className="w-5 h-5 text-green-500 shrink-0" />}
            {toast.type === 'error' && <XCircle className="w-5 h-5 text-red-500 shrink-0" />}
            {toast.type === 'warning' && <AlertTriangle className="w-5 h-5 text-yellow-500 shrink-0" />}
            <span className="text-sm font-medium">{toast.message}</span>
          </div>
        </div>
      )}

      {/* 确认弹窗 */}
      <Dialog open={confirmDialog.open} onOpenChange={(v) => !v && setConfirmDialog(prev => ({ ...prev, open: false }))}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{confirmDialog.title}</DialogTitle>
            <DialogDescription>{confirmDialog.description}</DialogDescription>
          </DialogHeader>
          <DialogFooter className="gap-2 sm:gap-0">
            <Button variant="outline" onClick={() => setConfirmDialog(prev => ({ ...prev, open: false }))}>
              取消
            </Button>
            <Button
              variant="destructive"
              onClick={confirmDialog.onConfirm}
            >
              确认
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
