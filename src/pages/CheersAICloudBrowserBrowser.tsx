import { CLOUD_APP_URL } from "@/lib/cloud";
import { getPlatform } from "@/lib/path";
import CheersAICloud from "@/pages/CheersAICloud";

const openExternalBrowser = () => {
  window.open(CLOUD_APP_URL, "_blank", "noopener,noreferrer");
};

/**
 * 普通浏览器宿主的 `/cloud` 页面：不尝试挂载 Tauri 子 WebView，不调用任何
 * Tauri 命令，始终停在统一回退页，改用系统浏览器新标签打开云端工作区。
 */
export default function CheersAICloudBrowserBrowser() {
  const isMacOS = getPlatform() === "macos";

  return (
    <CheersAICloud
      cloudUrl={CLOUD_APP_URL}
      mountState="fallback"
      mountError={null}
      isMacOS={isMacOS}
      onRetryEmbed={undefined}
      onOpenStandalone={openExternalBrowser}
      onOpenExternal={openExternalBrowser}
    />
  );
}
