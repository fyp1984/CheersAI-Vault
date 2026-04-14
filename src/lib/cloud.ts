export const CLOUD_ORIGIN = "https://uat-desktop.cheersai.cloud";
export const CLOUD_APP_PATH = "/";
export const CLOUD_APP_URL = `${CLOUD_ORIGIN}${CLOUD_APP_PATH}`;

export const CLOUD_LOGIN_URLS = [
  `${CLOUD_ORIGIN}/login`,
  `${CLOUD_ORIGIN}/cheersai_desktop/login`,
  `${CLOUD_ORIGIN}/auth/login`,
  `${CLOUD_ORIGIN}/user/login`,
];

export function resolveCloudUrl(path: string): string {
  if (path.startsWith("http://") || path.startsWith("https://")) {
    return path;
  }

  return `${CLOUD_ORIGIN}${path.startsWith("/") ? "" : "/"}${path}`;
}
