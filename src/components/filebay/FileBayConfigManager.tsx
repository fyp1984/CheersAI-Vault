import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { 
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { 
  FileText, 
  Upload, 
  Trash2, 
  CheckCircle2, 
  AlertTriangle,
  RefreshCw,
  ExternalLink
} from "lucide-react";
import { tauriCommands } from "@/lib/tauri";
import type { FileBayConfigStatus } from "@/types/commands";
import { open } from "@tauri-apps/plugin-dialog";
import { CLOUD_APP_URL } from "@/lib/cloud";

export function FileBayConfigManager() {
  const [configStatus, setConfigStatus] = useState<FileBayConfigStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [showDeleteDialog, setShowDeleteDialog] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);

  // 加载配置状态
  const loadConfigStatus = async () => {
    setLoading(true);
    try {
      const status = await tauriCommands.readFilebayConfig();
      setConfigStatus(status);
      
      // 如果检测到配置文件，显示提示
      if (status.exists && status.config) {
        setMessage({ 
          type: 'success', 
          text: `已自动检测到配置文件 (下载于 ${status.config.downloadedAt})` 
        });
      }
    } catch (error) {
      console.error("Failed to load FileBay config:", error);
      setMessage({ type: 'error', text: `加载配置失败: ${error}` });
    } finally {
      setLoading(false);
    }
  };

  // 导入配置文件
  const importConfig = async () => {
    try {
      const selected = await open({
        title: "选择 FileBay 配置文件",
        filters: [
          {
            name: "JSON 文件",
            extensions: ["json"]
          }
        ]
      });

      if (selected) {
        const filePath = selected as string;
        
        // 先验证文件
        try {
          await tauriCommands.validateFilebayConfigFile(filePath);
        } catch (error) {
          setMessage({ type: 'error', text: `配置文件验证失败: ${error}` });
          return;
        }

        // 导入文件
        const result = await tauriCommands.importFilebayConfig(filePath);
        setMessage({ type: 'success', text: result });
        
        // 重新加载状态
        await loadConfigStatus();
      }
    } catch (error) {
      console.error("Failed to import config:", error);
      setMessage({ type: 'error', text: `导入失败: ${error}` });
    }
  };

  // 删除配置文件
  const deleteConfig = async () => {
    try {
      const result = await tauriCommands.deleteFilebayConfig();
      setMessage({ type: 'success', text: result });
      setShowDeleteDialog(false);
      
      // 重新加载状态
      await loadConfigStatus();
    } catch (error) {
      console.error("Failed to delete config:", error);
      setMessage({ type: 'error', text: `删除失败: ${error}` });
    }
  };

  // 打开 Web App 下载页面
  const openDownloadPage = async () => {
    try {
      const { open: openExternal } = await import("@tauri-apps/plugin-shell");
      await openExternal(CLOUD_APP_URL);
      setMessage({
        type: 'info',
        text: '已打开 Desktop 在线工作区，请在云端页面中下载或刷新 FileBay 配置文件。',
      });
    } catch (error) {
      console.error("Failed to open cloud page:", error);
      setMessage({ type: 'error', text: `打开在线工作区失败: ${error}` });
    }
  };

  useEffect(() => {
    loadConfigStatus();
  }, []);

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <FileText className="w-5 h-5" />
          FileBay 配置管理
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* 状态显示 */}
        {configStatus?.exists ? (
          <div className="p-4 bg-blue-50 border border-blue-200 rounded-lg">
            <div className="flex items-center gap-2 mb-3">
              <CheckCircle2 className="w-5 h-5 text-blue-600" />
              <span className="font-semibold text-blue-800">配置已加载</span>
            </div>
            
            {configStatus.config && (
              <div className="space-y-2 text-sm">
                <div>
                  <span className="font-medium">服务器:</span> {configStatus.config.url}
                </div>
                <div>
                  <span className="font-medium">用户:</span> {configStatus.config.username}
                </div>
                <div>
                  <span className="font-medium">仓库:</span> {configStatus.config.repoName}
                </div>
                <div>
                  <span className="font-medium">邮箱:</span> {configStatus.config.email}
                </div>
                {configStatus.lastModified && (
                  <div>
                    <span className="font-medium">更新时间:</span> {configStatus.lastModified}
                  </div>
                )}
              </div>
            )}
          </div>
        ) : (
          <div className="p-4 bg-yellow-50 border border-yellow-200 rounded-lg">
            <div className="flex items-center gap-2 mb-2">
              <AlertTriangle className="w-5 h-5 text-yellow-600" />
              <span className="font-semibold text-yellow-800">未检测到配置文件</span>
            </div>
            <p className="text-sm text-yellow-700">
              请从 Desktop 在线工作区下载配置文件。配置文件会自动保存到 downloads 文件夹并被检测。
            </p>
          </div>
        )}

        {/* 消息显示 */}
        {message && (
          <div className={`p-3 rounded-lg ${
            message.type === 'success' ? 'bg-blue-50 border border-blue-200 text-blue-800' :
            message.type === 'error' ? 'bg-red-50 border border-red-200 text-red-800' :
            'bg-blue-50 border border-blue-200 text-blue-800'
          }`}>
            {message.text}
          </div>
        )}

        {/* 操作按钮 */}
        <div className="flex flex-wrap gap-2">
          <Button
            onClick={openDownloadPage}
            variant="outline"
            size="sm"
            className="flex items-center gap-2"
          >
            <ExternalLink className="w-4 h-4" />
            打开下载页面
          </Button>
          
          <Button
            onClick={importConfig}
            variant="outline"
            size="sm"
            className="flex items-center gap-2"
          >
            <Upload className="w-4 h-4" />
            导入配置文件
          </Button>
          
          <Button
            onClick={loadConfigStatus}
            variant="outline"
            size="sm"
            disabled={loading}
            className="flex items-center gap-2"
          >
            <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
            刷新状态
          </Button>
          
          {configStatus?.exists && (
            <Button
              onClick={() => setShowDeleteDialog(true)}
              variant="outline"
              size="sm"
              className="flex items-center gap-2 text-red-600 hover:text-red-700"
            >
              <Trash2 className="w-4 h-4" />
              删除配置
            </Button>
          )}
        </div>

        {/* 故障排查提示 */}
        <div className="p-3 bg-yellow-50 border border-yellow-200 rounded-lg">
          <p className="text-sm text-yellow-800">
            💡 如果下载没有反应的话，请重新点击 FileBay 配置文件或者手动上传重试。
          </p>
        </div>

        {/* 使用说明 */}
        <div className="p-4 bg-blue-50 border border-blue-200 rounded-lg">
          <h4 className="font-semibold text-blue-900 mb-2">使用说明</h4>
          <ol className="text-sm text-blue-800 space-y-1 list-decimal list-inside">
            <li>切换到 Desktop 在线工作区</li>
            <li>在 Web 页面中登录并配置 FileBay</li>
            <li>点击下载配置按钮，文件会自动保存到 downloads 文件夹</li>
            <li>返回此页面，点击"刷新状态"即可自动检测配置</li>
            <li>配置检测成功后即可在脱敏功能中使用</li>
          </ol>
        </div>

        {/* 删除确认对话框 */}
        <Dialog open={showDeleteDialog} onOpenChange={setShowDeleteDialog}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>确认删除</DialogTitle>
              <DialogDescription>
                确定要删除 FileBay 配置文件吗？删除后需要重新导入配置才能使用文件上传功能。
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button variant="outline" onClick={() => setShowDeleteDialog(false)}>
                取消
              </Button>
              <Button variant="destructive" onClick={deleteConfig}>
                删除
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </CardContent>
    </Card>
  );
}
