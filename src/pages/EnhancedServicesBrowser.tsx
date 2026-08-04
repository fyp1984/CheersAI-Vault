import { useState, useEffect } from 'react';
import { CheckCircle, AlertCircle, Loader2, Package, ShieldAlert, WifiOff } from 'lucide-react';
import { fetchRuntimeOcrStatus, type RuntimeFetchResult } from '@/lib/runtime/client';
import type { RuntimeOcrStatusResponse } from '@/types/runtime';

type BrowserOcrView =
  | { kind: 'loading' }
  | { kind: 'disconnected' }
  | { kind: 'status'; data: RuntimeOcrStatusResponse };

const OCR_STATUS_LABEL: Record<string, string> = {
  ready: 'OCR 可用',
  invalid: 'OCR 配置不完整',
  unavailable: 'OCR 未配置',
};

function toBrowserOcrView(result: RuntimeFetchResult<RuntimeOcrStatusResponse>): BrowserOcrView {
  if (!result.ok) {
    return { kind: 'disconnected' };
  }
  return { kind: 'status', data: result.data };
}

/**
 * 普通浏览器的增强服务页面：只读展示服务器 Runtime 的真实 OCR 状态，
 * 不提供安装/卸载/选目录等本机操作，也不获取或伪装未接入浏览器的
 * Ollama/AI 功能状态。
 */
export default function EnhancedServicesBrowser() {
  const [ocrView, setOcrView] = useState<BrowserOcrView>({ kind: 'loading' });

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      const result = await fetchRuntimeOcrStatus();
      if (!cancelled) {
        setOcrView(toBrowserOcrView(result));
      }
    };

    load();

    return () => {
      cancelled = true;
    };
  }, []);

  const statusTone =
    ocrView.kind === 'status' && ocrView.data.status === 'ready'
      ? 'bg-blue-100 text-blue-800'
      : ocrView.kind === 'disconnected'
        ? 'bg-gray-100 text-gray-600'
        : 'bg-amber-100 text-amber-800';

  return (
    <div className="p-6 w-full max-w-6xl mx-auto">
      <div className="mb-6">
        <h2 className="text-2xl font-bold text-gray-900 mb-2">增强服务</h2>
        <p className="text-gray-600">当前通过浏览器访问，增强服务由服务器统一管理。</p>
      </div>

      <div className="bg-white border border-gray-200 rounded-lg shadow-sm overflow-hidden max-w-2xl">
        <div className="p-6">
          <div className="flex items-start space-x-4">
            <div className="flex-shrink-0">
              <div className="w-12 h-12 bg-blue-100 rounded-lg flex items-center justify-center">
                <Package className="w-6 h-6 text-blue-600" />
              </div>
            </div>
            <div className="flex-1">
              <h3 className="text-lg font-semibold text-gray-900 mb-1">OCR 文字识别服务</h3>
              <p className="text-sm text-gray-600 mb-3">扫描件 PDF 的文字识别由服务器 Runtime 提供。</p>

              {ocrView.kind === 'loading' && (
                <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-600">
                  <Loader2 className="w-3 h-3 mr-1 animate-spin" />
                  正在查询服务器状态...
                </span>
              )}

              {ocrView.kind === 'disconnected' && (
                <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${statusTone}`}>
                  <WifiOff className="w-3 h-3 mr-1" />
                  无法连接服务器，请确认 Runtime 已启动
                </span>
              )}

              {ocrView.kind === 'status' && (
                <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${statusTone}`}>
                  {ocrView.data.status === 'ready' ? (
                    <CheckCircle className="w-3 h-3 mr-1" />
                  ) : (
                    <AlertCircle className="w-3 h-3 mr-1" />
                  )}
                  {OCR_STATUS_LABEL[ocrView.data.status] ?? `未知状态：${ocrView.data.status}`}
                </span>
              )}
            </div>
          </div>

          <div className="mt-4 pt-4 border-t border-gray-100 flex items-start gap-2 text-sm text-gray-600">
            <ShieldAlert className="w-4 h-4 mt-0.5 flex-shrink-0 text-blue-600" />
            <p>
              OCR 由服务器管理员统一安装和配置，浏览器端无需、也无法自行安装、卸载或选择安装目录；如需启用或排查，请联系管理员。
            </p>
          </div>
        </div>
      </div>

      <div className="mt-6 max-w-2xl p-4 bg-blue-50 border border-blue-200 rounded-lg text-sm text-blue-800">
        AI 智能脱敏模型（Ollama）尚未接入浏览器访问，暂不在此展示；如需使用，请在桌面客户端中安装和管理。
      </div>
    </div>
  );
}
