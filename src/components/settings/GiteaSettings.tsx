import { useState, useEffect } from 'react';
import { getGiteaStatus, updateGiteaConfig, createGiteaRepo, testGiteaConnection } from '../../services/gitea';
import type { GiteaStatusResponse } from '../../types/gitea';

export function GiteaSettings() {
  const [status, setStatus] = useState<GiteaStatusResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [creating, setCreating] = useState(false);
  const [testing, setTesting] = useState(false);
  
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
      // 只在初始加载时更新配置，保存后不清空用户输入
      setConfig(prev => ({
        url: result.config.url || prev.url,
        token: prev.token, // 保留用户输入的 token
        owner: result.config.owner || prev.owner,
        repo: result.config.repo || prev.repo,
        enabled: result.config.enabled || prev.enabled,
      }));
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
      alert('请填写完整的配置信息');
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
      alert('配置已保存');
    } catch (error) {
      console.error('Failed to save config:', error);
      alert('保存失败: ' + error);
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
      alert('✅ ' + result);
    } catch (error) {
      console.error('Connection test failed:', error);
      alert('❌ ' + error);
    } finally {
      setTesting(false);
    }
  };

  const handleCreateRepo = async () => {
    try {
      setCreating(true);
      const result = await createGiteaRepo(true);
      
      // 显示结果
      alert(result);
      
      // 刷新状态（在用户关闭弹窗后）
      await loadStatus();
    } catch (error) {
      console.error('Failed to create repo:', error);
      alert('创建仓库失败: ' + error);
      await loadStatus();
    } finally {
      setCreating(false);
    }
  };

  if (loading) {
    return (
      <div className="p-6">
        <div className="animate-pulse">
          <div className="h-8 bg-gray-200 rounded w-1/4 mb-4"></div>
          <div className="space-y-3">
            <div className="h-10 bg-gray-200 rounded"></div>
            <div className="h-10 bg-gray-200 rounded"></div>
            <div className="h-10 bg-gray-200 rounded"></div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 max-w-4xl mx-auto">
      <div className="mb-6">
        <h2 className="text-2xl font-bold text-gray-900 mb-2">FileBay 配置</h2>
        <p className="text-gray-600">
          配置 FileBay 服务器信息，将脱敏后的文件自动上传到 FileBay 仓库进行版本管理
        </p>
      </div>

      {/* 状态指示器 */}
      {status && (
        <div className="mb-6 p-4 rounded-lg border">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-3">
              <div className={`w-3 h-3 rounded-full ${
                status.enabled && status.configured && status.repo_exists 
                  ? 'bg-green-500' 
                  : status.enabled && status.configured 
                  ? 'bg-yellow-500' 
                  : 'bg-gray-300'
              }`}></div>
              <div>
                <div className="font-medium text-gray-900">
                  {status.enabled && status.configured && status.repo_exists
                    ? '已启用并就绪'
                    : status.enabled && status.configured
                    ? '已配置，需要创建仓库'
                    : status.enabled
                    ? '已启用，需要配置'
                    : '未启用'}
                </div>
                <div className="text-sm text-gray-500">
                  {status.configured ? '配置完整' : '请完成配置'}
                  {status.repo_exists !== null && (
                    <> · {status.repo_exists ? '仓库已存在' : '仓库未创建'}</>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 配置表单 */}
      <div className="space-y-4 mb-6">
        {/* 启用开关 */}
        <div className="flex items-center justify-between p-4 bg-gray-50 rounded-lg">
          <div>
            <label className="font-medium text-gray-900">启用 FileBay 上传</label>
            <p className="text-sm text-gray-500">脱敏完成后可选择上传到 FileBay</p>
          </div>
          <button
            type="button"
            onClick={handleToggleEnabled}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              config.enabled ? 'bg-blue-600' : 'bg-gray-200'
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                config.enabled ? 'translate-x-6' : 'translate-x-1'
              }`}
            />
          </button>
        </div>

        {/* Gitea URL */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">
            FileBay 服务器地址
          </label>
          <input
            type="url"
            value="https://uat-filebay.cheersai.cloud"
            disabled
            className="w-full px-3 py-2 border border-gray-200 rounded-md bg-gray-50 text-gray-600 cursor-not-allowed"
          />
        </div>

        {/* Access Token */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">
            访问令牌 (Access Token)
          </label>
          <div className="relative">
            <input
              type="password"
              value={config.token}
              onChange={(e) => setConfig({ ...config, token: e.target.value })}
              placeholder={status?.config.has_token ? "••••••••（已保存，如需修改请重新输入）" : "输入您的 FileBay Access Token"}
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
            {status?.config.has_token && !config.token && (
              <div className="absolute right-3 top-2.5">
                <span className="text-xs text-green-600 bg-green-50 px-2 py-1 rounded">✓ 已保存</span>
              </div>
            )}
          </div>
          <p className="mt-1 text-xs text-gray-500">
            在 FileBay 设置 → 应用 → 管理访问令牌 中生成。出于安全考虑，已保存的 Token 不会显示。
          </p>
        </div>

        {/* Owner */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">
            用户名 / 组织名
          </label>
          <input
            type="text"
            value={config.owner}
            onChange={(e) => setConfig({ ...config, owner: e.target.value })}
            placeholder="your-username"
            className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        {/* Repo */}
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">
            仓库名称
          </label>
          <input
            type="text"
            value={config.repo}
            onChange={(e) => setConfig({ ...config, repo: e.target.value })}
            placeholder="masked-files"
            className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          <p className="mt-1 text-xs text-gray-500">
            用于存储脱敏文件的仓库名称
          </p>
        </div>
      </div>

      {/* 操作按钮 */}
      <div className="flex items-center space-x-3">
        <button
          onClick={handleSave}
          disabled={saving}
          className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {saving ? '保存中...' : '保存配置'}
        </button>

        <button
          onClick={handleTestConnection}
          disabled={testing}
          className="px-4 py-2 bg-purple-600 text-white rounded-md hover:bg-purple-700 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {testing ? '测试中...' : '测试连接'}
        </button>

        {status?.configured && !status?.repo_exists && (
          <button
            onClick={handleCreateRepo}
            disabled={creating}
            className="px-4 py-2 bg-green-600 text-white rounded-md hover:bg-green-700 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            {creating ? '创建中...' : '创建仓库'}
          </button>
        )}

        <button
          onClick={loadStatus}
          className="px-4 py-2 border border-gray-300 text-gray-700 rounded-md hover:bg-gray-50"
        >
          刷新状态
        </button>
      </div>

      {/* 帮助信息 */}
      <div className="mt-8 space-y-4">
        <div className="p-4 bg-blue-50 border border-blue-200 rounded-lg">
          <h3 className="font-medium text-blue-900 mb-2">💡 配置说明</h3>
          <ul className="text-sm text-blue-800 space-y-1 list-disc list-inside">
            <li><strong>FileBay 服务器地址</strong>：完整的 FileBay 服务器 URL（例如：http://localhost:3000）</li>
            <li><strong>访问令牌</strong>：在 FileBay 设置 → 应用 → 管理访问令牌 中生成（需要 repo 权限）</li>
            <li><strong>用户名</strong>：你的 FileBay 用户名（不是邮箱）</li>
            <li><strong>仓库名称</strong>：用于存储脱敏文件的仓库名（如：masked-files）</li>
          </ul>
        </div>

        <div className="p-4 bg-yellow-50 border border-yellow-200 rounded-lg">
          <h3 className="font-medium text-yellow-900 mb-2">⚠️ 常见问题</h3>
          <ul className="text-sm text-yellow-800 space-y-1 list-disc list-inside">
            <li><strong>认证失败</strong>：请检查 Token 是否正确，是否有 repo 权限</li>
            <li><strong>URL 错误</strong>：确保 URL 格式正确，不要包含 /api 路径</li>
            <li><strong>用户名错误</strong>：使用 FileBay 用户名，不是显示名称</li>
            <li><strong>仓库已存在</strong>：如果仓库已存在，无需重复创建</li>
          </ul>
        </div>

        <div className="p-4 bg-green-50 border border-green-200 rounded-lg">
          <h3 className="font-medium text-green-900 mb-2">✅ 配置步骤</h3>
          <ol className="text-sm text-green-800 space-y-1 list-decimal list-inside">
            <li>登录你的 FileBay 服务器</li>
            <li>进入 设置 → 应用 → 管理访问令牌</li>
            <li>点击"生成新令牌"，选择 repo 权限</li>
            <li>复制生成的 Token 并填写到上方表单</li>
            <li>填写完整信息后点击"保存配置"</li>
            <li>点击"创建仓库"按钮（如果仓库不存在）</li>
          </ol>
        </div>

        <div className="p-4 bg-gray-50 border border-gray-200 rounded-lg">
          <h3 className="font-medium text-gray-900 mb-2">📝 配置示例</h3>
          <div className="text-sm text-gray-700 space-y-2 font-mono">
            <div><strong>URL:</strong> http://localhost:8080 或 http://localhost:3000</div>
            <div><strong>Token:</strong> 从 FileBay 设置中生成的令牌</div>
            <div><strong>用户名:</strong> 你的 FileBay 用户名</div>
            <div><strong>仓库:</strong> masked-files</div>
          </div>
          <div className="mt-3 text-xs text-gray-600">
            💡 提示：URL 端口取决于你的 FileBay 配置，常见端口有 3000、3001、8080
          </div>
        </div>
      </div>
    </div>
  );
}
