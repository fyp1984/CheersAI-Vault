/**
 * Vault 配置选择器组件
 * 
 * 从 Vault Bridge 数据库加载并选择 FileBay 配置
 * 遵循 CheersAI UI 规范 v1.0
 */

import { useEffect, useState } from 'react';
import { 
  listVaultConfigs, 
  checkVaultDbExists, 
  getVaultDbPath, 
  getVaultDbStats,
  VaultFileBayConfig,
  VaultDbStats 
} from '@/lib/vault';
import { RefreshCw, CheckCircle, AlertTriangle, Database } from 'lucide-react';

interface VaultConfigSelectorProps {
  onConfigSelected: (config: VaultFileBayConfig) => void;
  autoSelectSingle?: boolean;
}

export function VaultConfigSelector({ 
  onConfigSelected, 
  autoSelectSingle = true 
}: VaultConfigSelectorProps) {
  const [configs, setConfigs] = useState<VaultFileBayConfig[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [stats, setStats] = useState<VaultDbStats | null>(null);
  const [selectedConfig, setSelectedConfig] = useState<VaultFileBayConfig | null>(null);

  useEffect(() => {
    loadConfigs();
  }, []);

  const loadConfigs = async () => {
    setLoading(true);
    setError(null);
    
    try {
      // 获取数据库统计信息
      const dbStats = await getVaultDbStats();
      setStats(dbStats);
      
      if (!dbStats.exists) {
        setError('Vault 数据库不存在');
        return;
      }
      
      if (dbStats.config_count === 0) {
        setError('数据库中没有配置');
        return;
      }
      
      // 加载配置列表
      const configList = await listVaultConfigs();
      setConfigs(configList);
      
      // 如果只有一个配置且启用自动选择，自动选择它
      if (configList.length === 1 && autoSelectSingle) {
        setSelectedConfig(configList[0]);
        onConfigSelected(configList[0]);
      }
    } catch (err: any) {
      console.error('Failed to load Vault configs:', err);
      setError(err.toString());
    } finally {
      setLoading(false);
    }
  };

  const handleSelectConfig = (config: VaultFileBayConfig) => {
    setSelectedConfig(config);
    onConfigSelected(config);
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center p-8">
        <RefreshCw className="w-8 h-8 text-primary animate-spin" />
        <span className="ml-3 text-sm text-gray-600">正在加载 Vault 配置...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6 bg-error/5 border border-error/20 rounded-lg">
        <div className="flex items-start gap-3 mb-4">
          <AlertTriangle className="w-5 h-5 text-error flex-shrink-0 mt-0.5" />
          <div className="flex-1">
            <h3 className="text-base font-semibold text-gray-900 mb-1">无法加载配置</h3>
            <p className="text-sm text-error">{error}</p>
          </div>
        </div>
        
        {stats && (
          <div className="bg-white p-4 rounded-lg border border-gray-200 mb-4">
            <div className="flex items-center gap-2 mb-2">
              <Database className="w-4 h-4 text-gray-500" />
              <span className="text-xs font-medium text-gray-700 uppercase tracking-wide">数据库信息</span>
            </div>
            <div className="space-y-1.5 text-sm">
              <div className="flex items-center justify-between">
                <span className="text-gray-600">路径:</span>
                <code className="text-xs bg-gray-100 px-2 py-0.5 rounded text-gray-700">{stats.path}</code>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-gray-600">状态:</span>
                <span className={`text-xs font-medium ${stats.exists ? 'text-success' : 'text-error'}`}>
                  {stats.exists ? '✓ 存在' : '✗ 不存在'}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span className="text-gray-600">配置数量:</span>
                <span className="text-xs font-medium text-gray-900">{stats.config_count}</span>
              </div>
            </div>
          </div>
        )}
        
        <div className="bg-warning/10 border-l-4 border-warning p-4 mb-4 rounded">
          <h4 className="text-sm font-semibold text-gray-900 mb-2 flex items-center gap-1.5">
            <AlertTriangle className="w-4 h-4 text-warning" />
            解决步骤
          </h4>
          <ol className="list-decimal list-inside space-y-2 text-sm text-gray-700">
            <li>
              打开浏览器访问: 
              <code className="bg-warning/20 px-2 py-0.5 rounded ml-1 text-xs">http://localhost:3000/signin</code>
            </li>
            <li>使用 Desktop SSO 登录</li>
            <li>
              访问同步页面: 
              <code className="bg-warning/20 px-2 py-0.5 rounded ml-1 text-xs">http://localhost:3000/sync-config</code>
            </li>
            <li>点击 <strong>"开始同步"</strong> 按钮</li>
            <li>等待同步完成</li>
            <li>返回此应用，点击下面的 <strong>"重新加载"</strong> 按钮</li>
          </ol>
        </div>
        
        <button
          onClick={loadConfigs}
          className="w-full bg-primary text-white py-2.5 px-4 rounded-lg hover:bg-primary-dark transition-colors duration-200 font-medium text-sm flex items-center justify-center gap-2"
        >
          <RefreshCw className="w-4 h-4" />
          重新加载
        </button>
      </div>
    );
  }

  if (configs.length === 0) {
    return (
      <div className="p-6 bg-gray-50 border border-gray-200 rounded-lg">
        <p className="text-sm text-gray-600 mb-4">没有找到配置</p>
        <button
          onClick={loadConfigs}
          className="w-full bg-primary text-white py-2.5 px-4 rounded-lg hover:bg-primary-dark transition-colors duration-200 font-medium text-sm flex items-center justify-center gap-2"
        >
          <RefreshCw className="w-4 h-4" />
          重新加载
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between mb-4">
        <div>
          <h3 className="text-lg font-semibold text-gray-900">选择 FileBay 配置</h3>
          {stats && (
            <p className="text-sm text-gray-600 mt-1">
              找到 {stats.config_count} 个配置
              {stats.last_updated && (
                <span className="ml-2 text-gray-500">
                  · 最后更新: {new Date(stats.last_updated).toLocaleString('zh-CN')}
                </span>
              )}
            </p>
          )}
        </div>
        <button
          onClick={loadConfigs}
          className="text-sm text-primary hover:text-primary-dark font-medium flex items-center gap-1.5 transition-colors duration-200"
        >
          <RefreshCw className="w-4 h-4" />
          刷新
        </button>
      </div>
      
      <div className="space-y-3">
        {configs.map((config) => {
          const isSelected = selectedConfig?.user_id === config.user_id;
          
          return (
            <div
              key={config.user_id}
              onClick={() => handleSelectConfig(config)}
              className={`p-4 border-2 rounded-lg cursor-pointer transition-all duration-200 ${
                isSelected
                  ? 'border-primary bg-primary/5 shadow-md'
                  : 'border-gray-200 bg-white hover:border-primary/50 hover:shadow-sm'
              }`}
            >
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2">
                  {isSelected && <CheckCircle className="w-5 h-5 text-primary" />}
                  <span className="font-semibold text-gray-900">{config.email}</span>
                </div>
                <span className="text-sm text-gray-500 bg-gray-100 px-2 py-0.5 rounded-full">
                  @{config.username}
                </span>
              </div>
              <div className="space-y-2 text-sm">
                <div className="flex items-start gap-2">
                  <span className="font-medium text-gray-600 min-w-[60px]">URL:</span>
                  <span className="break-all text-gray-900">{config.url}</span>
                </div>
                <div className="flex items-start gap-2">
                  <span className="font-medium text-gray-600 min-w-[60px]">仓库:</span>
                  <span className="text-gray-900">{config.repo_name}</span>
                </div>
                <div className="flex items-start gap-2">
                  <span className="font-medium text-gray-600 min-w-[60px]">更新:</span>
                  <span className="text-gray-700">{new Date(config.updated_at).toLocaleString('zh-CN')}</span>
                </div>
              </div>
            </div>
          );
        })}
      </div>
      
      {stats && (
        <div className="text-xs text-gray-500 mt-4 p-3 bg-gray-50 rounded-lg border border-gray-200">
          <div className="flex items-center gap-2 mb-1">
            <Database className="w-3.5 h-3.5" />
            <span className="font-medium">数据库位置</span>
          </div>
          <div className="break-all font-mono">{stats.path}</div>
        </div>
      )}
    </div>
  );
}
