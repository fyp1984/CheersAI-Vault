import { lazy, Suspense } from "react";
import { isTauriHost } from "@/lib/runtime/host";
import { Loading } from "@/components/ui/cheersai-ui";

const EnhancedServicesDesktop = lazy(() => import("./EnhancedServicesDesktop"));
const EnhancedServicesBrowser = lazy(() => import("./EnhancedServicesBrowser"));

/**
 * `/enhanced` 的宿主入口：单一判定，桌面走原有 Tauri 安装/卸载管理界面
 * （逻辑与行为完全不变，机械迁移至 EnhancedServicesDesktop.tsx），浏览器走
 * 只读的 Runtime OCR 状态展示（EnhancedServicesBrowser.tsx）。
 */
export default function EnhancedServices() {
  const isDesktop = isTauriHost();
  return (
    <Suspense fallback={<div className="flex h-full items-center justify-center"><Loading text="正在加载…" /></div>}>
      {isDesktop ? <EnhancedServicesDesktop /> : <EnhancedServicesBrowser />}
    </Suspense>
  );
}
