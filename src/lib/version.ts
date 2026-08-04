import { isTauriHost } from "@/lib/runtime/host";

const buildVersion = import.meta.env.VITE_APP_VERSION ?? "0.0.0";

export async function getAppVersion() {
  // 浏览器宿主没有 Tauri app 版本可查，直接使用构建版本，不尝试加载 Tauri app API。
  if (!isTauriHost()) {
    return buildVersion;
  }

  try {
    const { getVersion } = await import("@tauri-apps/api/app");
    return await getVersion();
  } catch {
    return buildVersion;
  }
}

export function getBuildVersion() {
  return buildVersion;
}
