import { getVersion } from "@tauri-apps/api/app";

const buildVersion = import.meta.env.VITE_APP_VERSION ?? "0.0.0";

export async function getAppVersion() {
  try {
    return await getVersion();
  } catch {
    return buildVersion;
  }
}

export function getBuildVersion() {
  return buildVersion;
}
