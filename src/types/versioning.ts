export type ReleaseNoteKind = "feature" | "fix" | "security" | "breaking" | "ops";

export interface ReleaseNoteItem {
  kind: ReleaseNoteKind;
  text: string;
}

export interface VersionServiceDesktopConfig {
  enabled: boolean;
  pollIntervalMinutes: number;
  remindAfterHours: number;
  backupRequired: boolean;
  updateManifestUrl: string;
  releasePageUrl: string;
}

export interface VersionServiceWebConfig {
  pollIntervalMinutes: number;
  releasePageUrl: string;
}

export interface VersionServicePayload {
  product: string;
  channel: string;
  latestVersion: string;
  minimumSupportedVersion: string;
  publishedAt: string;
  releaseNotesSummary: string;
  releaseNotes: ReleaseNoteItem[];
  desktop: VersionServiceDesktopConfig;
  web: VersionServiceWebConfig;
}

export interface ResolvedVersionStatus {
  currentVersion: string;
  latestVersion: string;
  minimumSupportedVersion: string;
  updateAvailable: boolean;
  forceUpdate: boolean;
  releaseLevel: "same" | "patch" | "minor" | "major";
}
