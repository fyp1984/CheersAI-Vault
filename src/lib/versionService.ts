import type { ReleaseNoteItem, ReleaseNoteKind, VersionServicePayload } from "@/types/versioning";

const DEFAULT_VERSION_SERVICE_URL =
  (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env
    ?.VITE_VERSION_SERVICE_URL ??
  "https://raw.githubusercontent.com/fyp1984/CheersAI-Vault/main/releases/stable/version-info.json";

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isStringArrayOfObjects(value: unknown): value is Array<Record<string, unknown>> {
  return Array.isArray(value) && value.every((item) => isObject(item));
}

function isReleaseNoteKind(value: unknown): value is ReleaseNoteKind {
  return ["feature", "fix", "security", "breaking", "ops"].includes(String(value));
}

function isReleaseNoteItem(value: unknown): value is ReleaseNoteItem {
  return isObject(value) && isReleaseNoteKind(value.kind) && typeof value.text === "string";
}

export function getVersionServiceUrl(): string {
  return DEFAULT_VERSION_SERVICE_URL;
}

export function parseVersionServicePayload(payload: unknown): VersionServicePayload {
  if (!isObject(payload)) {
    throw new Error("版本服务响应格式无效");
  }

  const latestVersion = payload.latestVersion;
  const minimumSupportedVersion = payload.minimumSupportedVersion;
  const releaseNotes = payload.releaseNotes;
  const desktop = payload.desktop;
  const web = payload.web;

  if (
    typeof payload.product !== "string" ||
    typeof payload.channel !== "string" ||
    typeof latestVersion !== "string" ||
    typeof minimumSupportedVersion !== "string" ||
    typeof payload.publishedAt !== "string" ||
    typeof payload.releaseNotesSummary !== "string" ||
    !Array.isArray(releaseNotes) ||
    !isObject(desktop) ||
    !isObject(web)
  ) {
    throw new Error("版本服务缺少必要字段");
  }

  if (
    !Array.isArray(releaseNotes) ||
    !releaseNotes.every(isReleaseNoteItem) ||
    typeof desktop.enabled !== "boolean" ||
    typeof desktop.pollIntervalMinutes !== "number" ||
    typeof desktop.remindAfterHours !== "number" ||
    typeof desktop.backupRequired !== "boolean" ||
    typeof desktop.updateManifestUrl !== "string" ||
    typeof desktop.releasePageUrl !== "string" ||
    typeof web.pollIntervalMinutes !== "number" ||
    typeof web.releasePageUrl !== "string"
  ) {
    throw new Error("版本服务字段类型无效");
  }

  return {
    product: payload.product,
    channel: payload.channel,
    latestVersion,
    minimumSupportedVersion,
    publishedAt: payload.publishedAt,
    releaseNotesSummary: payload.releaseNotesSummary,
    releaseNotes,
    desktop: {
      enabled: desktop.enabled,
      pollIntervalMinutes: desktop.pollIntervalMinutes,
      remindAfterHours: desktop.remindAfterHours,
      backupRequired: desktop.backupRequired,
      updateManifestUrl: desktop.updateManifestUrl,
      releasePageUrl: desktop.releasePageUrl,
    },
    web: {
      pollIntervalMinutes: web.pollIntervalMinutes,
      releasePageUrl: web.releasePageUrl,
    },
  };
}

export async function fetchVersionService(
  signal?: AbortSignal,
  url = DEFAULT_VERSION_SERVICE_URL
): Promise<VersionServicePayload> {
  const response = await fetch(url, {
    method: "GET",
    headers: {
      Accept: "application/json",
    },
    signal,
  });

  if (!response.ok) {
    throw new Error(`版本服务请求失败: HTTP ${response.status}`);
  }

  const payload = await response.json();
  return parseVersionServicePayload(payload);
}
