import { useState, useEffect } from 'react';
import { getGiteaStatus, updateGiteaConfig, createGiteaRepo, testGiteaConnection } from '../../services/gitea';
import type { GiteaStatusResponse } from '../../types/gitea';
import { tauriCommands } from '@/lib/tauri';
import { VaultConfigSelector } from '../VaultConfigSelector';
import type { VaultFileBayConfig } from '@/lib/vault';
import { Button, Input, Message, Switch, Badge, Card } from '../ui/cheersai-ui';
import { RefreshCw, Database, Lock, Upload, Download, FileJson, Lightbulb, AlertTriangle, CheckCircle, Eye, EyeOff } from 'lucide-react';

export function GiteaSettings() {
  const [status, setStatus] = useState<GiteaStatusResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [creating, setCreating] = useState(false);
  const [testing, setTesting] = useState(false);
  const [autoLoading, setAutoLoading] = useState(false);
  const [autoSyncing, setAutoSyncing] = useState(false);
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);
  const [showVaultSelector, setShowVaultSelector] = useState(false);
  const [showToken, setShowToken] = useState(false);
  
  const handleToggleShowToken = async () => {
    if (!showToken) {
      // 如果要显示 Token，从数据库读取最新的 Token
      try {
        console.log('🔍 正在从数据库读取 Token...');
        const token = await tauriCommands.getFilebayToken();
        console.log('✅ Token 读取成功，长度:', token.length);
        console.log('🔑 Token 内容:', token);
        setConfig(prev => ({ ...prev, token }));
        setShowToken(true);
      } catch (error) {
        console.error('❌ 读取 Token 失败:', error);
        setShowToken(true); // 即使失败也切换显示状态
      }
    } else {
      // 隐藏时不清空 token，只是切换显示状态
      setShowToken(false);
    }
  };
  
  const [config, setConfig] = useState({
    url: 'https://uat-filebay.cheersai.cloud',
    token: '',
    owner: '',
    repo: '',
    enabled: false,
  });

  useEffect(() => {
    loadStatus();
  }, []);

  const loadStatus = async () => {
    try {
      setLoading(true);
      const result = await getGiteaStatus();
      setStatus(result);
      
      // 如果数据库中有 token，自动加载
      let tokenFromDb = '';
      if (result.config.has_token) {
        try {
          tokenFromDb = await tauriCommands.getFilebayToken();
          console.log('✅ 从数据库加载 Token，长度:', tokenFromDb.length);
        } catch (error) {
          console.error('❌ 从数据库加载 Token 失败:', error);
        }
      }
      
      // 更新配置
      setConfig(prev => ({
        url: result.config.url || prev.url,
        token: tokenFromDb || prev.token, // 使用数据库中的 token
        owner: result.config.owner || prev.owner,
        repo: result.config.repo || prev.repo,
        enabled: result.config.enabled || prev.enabled,
      }));
      
      // 如果有 token，自动显示
      if (tokenFromDb) {
        setShowToken(true);
      }
    } catch (error) {
      console.error('Failed to load Gitea status:', error);
      // 设置默认状态，避免一直加载
      setStatus({
        enabled: false,
        configured: false,
        repo_exists: null,
        config: {
          url: 'https://uat-filebay.cheersai.cloud',
          token: '',
          owner: '',
          repo: '',
          enabled: false,
          has_token: false,
        }
      });
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    const tokenProvided = !!config.token;
    const tokenAlreadySaved = !!status?.config.has_token;
    if (!config.url || (!tokenProvided && !tokenAlreadySaved) || !config.owner || !config.repo) {
      setMessage({ type: 'error', text: '请填写完整的配置信息' });
      return;
    }

    try {
      setSaving(true);
      await updateGiteaConfig({
        url: config.url,
        ...(tokenProvided ? { token: config.token } : {}),
        owner: config.owner,
        repo: config.repo,
        enabled: config.enabled,
      });
      
      const result = await getGiteaStatus();
      setStatus(result);
      setConfig(prev => ({ ...prev, enabled: result.config.enabled }));
      setMessage({ type: 'success', text: '配置已保存' });
    } catch (error) {
      console.error('Failed to save config:', error);
      setMessage({ type: 'error', text: '保存失败: ' + error });
    } finally {
      setSaving(false);
    }
  };

  const handleToggleEnabled = async () => {
    const newEnabled = !config.enabled;
    setConfig({ ...config, enabled: newEnabled });
    try {
      await updateGiteaConfig({ enabled: newEnabled });
      const result = await getGiteaStatus();
      setStatus(result);
    } catch (error) {
      console.error('Failed to toggle enabled:', error);
      setConfig({ ...config, enabled: !newEnabled });
    }
  };

  const handleTestConnection = async () => {
    try {
      setTesting(true);
      const result = await testGiteaConnection();
      setMessage({ type: 'success', text: '连接成功: ' + result });
    } catch (error) {
      console.error('Connection test failed:', error);
      setMessage({ type: 'error', text: '连接失败: ' + error });
    } finally {
      setTesting(false);
    }
  };

  const handleCreateRepo = async () => {
    try {
      setCreating(true);
      const result = await createGiteaRepo(true);
      
      // 显示结果
      setMessage({ type: 'success', text: result });
      
      // 刷新状态
      await loadStatus();
    } catch (error) {
      console.error('Failed to create repo:', error);
      setMessage({ type: 'error', text: '创建仓库失败: ' + error });
      await loadStatus();
    } finally {
      setCreating(false);
    }
  };
  
  // 一键读取配置文件
  const handleAutoLoadConfig = async () => {
    try {
      setAutoLoading(true);
      setMessage(null);
      const configStatus = await tauriCommands.readFilebayConfig();
      
      if (!configStatus.exists || !configStatus.config) {
        setMessage({ 
          type: 'error', 
          text: '未检测到配置文件。请先从 Desktop 在线工作区下载配置文件到浏览器的 Downloads 文件夹，然后点击此按钮自动读取' 
        });
        return;
      }
      
      const { config: fileConfig } = configStatus;
      
      // 自动填充表单
      setConfig({
        url: fileConfig.url || 'https://uat-filebay.cheersai.cloud',
        token: fileConfig.token || '',
        owner: fileConfig.username || '',
        repo: fileConfig.repoName || '',
        enabled: config.enabled, // 保持当前启用状态
      });
      
      setMessage({ 
        type: 'success', 
        text: `配置已自动读取：服务器 ${fileConfig.url}，用户 ${fileConfig.username}，仓库 ${fileConfig.repoName}。请点击"保存配置"以应用更改` 
      });
    } catch (error) {
      console.error('Failed to auto-load config:', error);
      setMessage({ 
        type: 'error', 
        text: `读取配置失败: ${error}` 
      });
    } finally {
      setAutoLoading(false);
    }
  };

  // 导入配置文件
  const handleImportConfig = async () => {
    try {
      setMessage(null);
      
      // 使用 Tauri 文件选择器
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'JSON',
          extensions: ['json']
        }],
        title: '选择 FileBay 配置文件',
      });
      
      if (!selected) {
        return; // 用户取消选择
      }
      
      const filePath = selected as string;
      console.log('Selected file path:', filePath);
      
      // 导入配置文件
      const result = await tauriCommands.importFilebayConfig(filePath);
      
      setMessage({ 
        type: 'success', 
        text: result 
      });
      
      // 导入成功后自动读取配置
      setTimeout(() => {
        handleAutoLoadConfig();
      }, 500);
      
    } catch (error) {
      console.error('Failed to import config:', error);
      console.error('Error details:', JSON.stringify(error));
      setMessage({ 
        type: 'error', 
        text: `导入失败: ${error}` 
      });
    }
  };

  // 从 Vault 加载配置
  const handleVaultConfigSelected = (vaultConfig: VaultFileBayConfig) => {
    setConfig({
      url: vaultConfig.url,
      token: vaultConfig.token,
      owner: vaultConfig.username,
      repo: vaultConfig.repo_name,
      enabled: config.enabled,
    });
    
    setMessage({
      type: 'success',
      text: `已从 Vault 加载配置：${vaultConfig.email} - ${vaultConfig.repo_name}。请点击"保存配置"以应用更改`
    });
    
    setShowVaultSelector(false);
  };
  
  // 自动从 Desktop 提取配置
  const handleAutoSync = async () => {
    try {
      setAutoSyncing(true);
      setMessage(null);
      
      // 调用 Tauri 命令从 Desktop webview 提取配置
      const result = await tauriCommands.extractConfigFromDesktopWebview();
      
      setMessage({
        type: 'success',
        text: result
      });
      
      // 等待一会儿让配置同步完成
      setTimeout(() => {
        loadStatus();
      }, 1000);
      
    } catch (error) {
      console.error('Failed to auto-sync:', error);
      setMessage({
        type: 'error',
        text: `自动同步失败: ${error}`
      });
    } finally {
      setAutoSyncing(false);
    }
  };

  if (loading) {
    return (
      <div className="p-8">
        <div className="animate-pulse space-y-4">
          <div className="h-8 bg-gray-200 rounded-lg w-1/4"></div>
          <div className="h-4 bg-gray-100 rounded w-1/2"></div>
          <div className="space-y-3 mt-6">
            <div className="h-12 bg-gray-200 rounded-lg"></div>
            <div className="h-12 bg-gray-200 rounded-lg"></div>
            <div className="h-12 bg-gray-200 rounded-lg"></div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="p-8 max-w-5xl mx-auto">
      <div className="mb-8">
        <h2 className="text-3xl font-bold text-gray-900 mb-2">FileBay 配置</h2>
        <p className="text-base text-gray-600">
          配置 FileBay 服务器信息，将脱敏后的文件自动上传到 FileBay 仓库进行版本管理
        </p>
      </div>

      {/* 状态指示器 */}
      {status && (
        <Card className="mb-6 p-5">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-4">
              <div className={`w-3 h-3 rounded-full ${
                status.enabled && status.configured && status.repo_exists 
                  ? 'bg-success animate-pulse' 
                  : status.enabled && status.configured 
                  ? 'bg-warning' 
                  : 'bg-gray-300'
              }`}></div>
              <div>
                <div className="font-semibold text-gray-900 text-base">
                  {status.enabled && status.configured && status.repo_exists
                    ? '已启用并就绪'
                    : status.enabled && status.configured
                    ? '已配置，需要创建仓库'
                    : status.enabled
                    ? '已启用，需要配置'
                    : '未启用'}
                </div>
                <div className="text-sm text-gray-600 mt-1">
                  {status.configured ? '配置完整' : '请完成配置'}
                  {status.repo_exists !== null && (
                    <> · {status.repo_exists ? '仓库已存在' : '仓库未创建'}</>
                  )}
                </div>
              </div>
            </div>
            <div className="flex gap-2">
              {status.enabled && status.configured && status.repo_exists && (
                <Badge variant="success">就绪</Badge>
              )}
              {status.enabled && status.configured && !status.repo_exists && (
                <Badge variant="warning">待创建</Badge>
              )}
              {!status.configured && (
                <Badge variant="neutral">未配置</Badge>
              )}
            </div>
          </div>
        </Card>
      )}

      {/* 消息提示 */}
      {message && (
        <Message
          type={message.type}
          onClose={() => setMessage(null)}
          className="mb-6"
        >
          {message.text}
        </Message>
      )}

      {/* 配置表单 */}
      <Card className="p-6 mb-6">
        <div className="space-y-6">
          {/* 启用开关 */}
          <Switch
            checked={config.enabled}
            onChange={handleToggleEnabled}
            label="启用 FileBay 上传"
            description="脱敏完成后可选择上传到 FileBay"
          />

          {/* Gitea URL */}
          <Input
            label="FileBay 服务器地址"
            value="https://uat-filebay.cheersai.cloud"
            disabled
          />

          {/* Access Token */}
          <div>
            <div className="relative">
              <Input
                label="访问令牌 (Access Token)"
                type={showToken ? "text" : "password"}
                value={config.token}
                onChange={(e) => setConfig({ ...config, token: e.target.value })}
                placeholder={status?.config.has_token ? "••••••••（已保存，如需修改请重新输入）" : "输入您的 FileBay Access Token"}
                helperText="在 FileBay 设置 → 应用 → 管理访问令牌 中生成。出于安全考虑，已保存的 Token 不会显示。"
              />
              <button
                type="button"
                onClick={handleToggleShowToken}
                className="absolute right-3 top-9 text-gray-500 hover:text-gray-700 transition-colors"
                title={showToken ? "隐藏 Token" : "显示 Token"}
              >
                {showToken ? <EyeOff className="w-5 h-5" /> : <Eye className="w-5 h-5" />}
              </button>
            </div>
            {status?.config.has_token && !config.token && (
              <div className="mt-2 flex items-center gap-2">
                <Badge variant="success">✓ Token 已保存</Badge>
                <span className="text-sm text-gray-600">
                  （出于安全考虑不显示，如需修改请重新输入）
                </span>
              </div>
            )}
          </div>

          {/* Owner */}
          <Input
            label="用户名 / 组织名"
            value={config.owner}
            onChange={(e) => setConfig({ ...config, owner: e.target.value })}
            placeholder="your-username"
          />

          {/* Repo */}
          <Input
            label="仓库名称"
            value={config.repo}
            onChange={(e) => setConfig({ ...config, repo: e.target.value })}
            placeholder="masked-files"
            helperText="用于存储脱敏文件的仓库名称"
          />
        </div>
      </Card>

      {/* 操作按钮 */}
      <div className="flex items-center gap-3 flex-wrap mb-6">
        <Button
          variant="primary"
          icon={FileJson}
          onClick={handleImportConfig}
        >
          手动导入配置
        </Button>
        
        <Button
          variant="primary"
          icon={Database}
          onClick={handleSave}
          loading={saving}
        >
          保存配置
        </Button>
      </div>

      {/* Vault 配置选择器 */}
      {showVaultSelector && (
        <Card className="mt-6 p-6 bg-info/5 border-2 border-info/20">
          <VaultConfigSelector onConfigSelected={handleVaultConfigSelected} />
        </Card>
      )}

      {/* 帮助信息 */}
      <div className="mt-8">
        <div className="flex items-start gap-3 p-4 bg-success/5 border border-success/20 rounded-lg">
          <CheckCircle className="w-5 h-5 text-success flex-shrink-0 mt-0.5" />
          <div className="flex-1">
            <h3 className="font-semibold text-success mb-2">快速配置</h3>
            <ol className="space-y-2 text-sm text-gray-700 list-decimal list-inside">
              <li>点击左侧"CheersAI"按钮，进入 Desktop 在线工作区</li>
              <li>在 Desktop 页面访问 FileBay 设置页面</li>
              <li>点击"下载配置文件"按钮</li>
              <li>配置会自动同步到本程序，无需手动操作</li>
            </ol>
          </div>
        </div>
      </div>
    </div>
  );
}
