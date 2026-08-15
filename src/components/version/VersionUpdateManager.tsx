import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Download, ExternalLink, RefreshCw, ShieldCheck } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Badge, Message } from "@/components/ui/cheersai-ui";
import { getAppVersion } from "@/lib/version";
import { fetchVersionService } from "@/lib/versionService";
import {
  compareVersions,
  formatDisplayVersion,
  postponeUntil,
  resolveVersionStatus,
} from "@/lib/versionPolicy";
import { isTauriHost } from "@/lib/runtime/host";
import { tauriCommands } from "@/lib/tauri";
import type { VersionServicePayload } from "@/types/versioning";

const REMINDER_KEY = "cheersai:version-reminder";
const DEFAULT_POLL_MS = 30 * 60 * 1000;

type InstallPhase =
  | "idle"
  | "backing-up"
  | "checking"
  | "downloading"
  | "ready-to-restart"
  | "error";

interface ReminderState {
  version: string;
  until: number;
}

function readReminder(): ReminderState | null {
  try {
    const raw = localStorage.getItem(REMINDER_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as ReminderState;
    if (typeof parsed.version !== "string" || typeof parsed.until !== "number") {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

function writeReminder(version: string, until: number) {
  try {
    localStorage.setItem(REMINDER_KEY, JSON.stringify({ version, until }));
  } catch {
    // ignore storage failures
  }
}

function clearReminder() {
  try {
    localStorage.removeItem(REMINDER_KEY);
  } catch {
    // ignore storage failures
  }
}

function shouldPrompt(version: string, forceUpdate: boolean): boolean {
  if (forceUpdate) return true;
  const reminder = readReminder();
  if (!reminder) return true;
  if (reminder.version !== version) return true;
  return reminder.until <= Date.now();
}

function openExternal(url: string) {
  window.open(url, "_blank", "noopener,noreferrer");
}

export function VersionUpdateManager() {
  const isDesktop = isTauriHost();
  const [currentVersion, setCurrentVersion] = useState("0.0.0");
  const [servicePayload, setServicePayload] = useState<VersionServicePayload | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [installPhase, setInstallPhase] = useState<InstallPhase>("idle");
  const [installError, setInstallError] = useState<string | null>(null);
  const [backupPath, setBackupPath] = useState<string | null>(null);
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [contentLength, setContentLength] = useState(0);
  const pollTimeoutRef = useRef<number | null>(null);

  const versionStatus = useMemo(() => {
    if (!servicePayload) return null;
    return resolveVersionStatus(
      currentVersion,
      servicePayload.latestVersion,
      servicePayload.minimumSupportedVersion
    );
  }, [currentVersion, servicePayload]);

  const scheduleNextCheck = useCallback((delayMs: number, task: () => Promise<void>) => {
    if (pollTimeoutRef.current !== null) {
      window.clearTimeout(pollTimeoutRef.current);
    }
    pollTimeoutRef.current = window.setTimeout(() => {
      void task();
    }, delayMs);
  }, []);

  const checkForUpdates = useCallback(async () => {
    try {
      const version = await getAppVersion();
      setCurrentVersion(version);

      const payload = await fetchVersionService();
      setServicePayload(payload);

      const status = resolveVersionStatus(
        version,
        payload.latestVersion,
        payload.minimumSupportedVersion
      );
      if (status.updateAvailable && shouldPrompt(payload.latestVersion, status.forceUpdate)) {
        setDialogOpen(true);
      }

      const minutes = Math.max(
        5,
        isDesktop ? payload.desktop.pollIntervalMinutes : payload.web.pollIntervalMinutes
      );
      scheduleNextCheck(minutes * 60 * 1000, checkForUpdates);
    } catch {
      scheduleNextCheck(DEFAULT_POLL_MS, checkForUpdates);
    }
  }, [isDesktop, scheduleNextCheck]);

  useEffect(() => {
    void checkForUpdates();

    const onVisibility = () => {
      if (document.visibilityState === "visible") {
        void checkForUpdates();
      }
    };
    document.addEventListener("visibilitychange", onVisibility);

    return () => {
      document.removeEventListener("visibilitychange", onVisibility);
      if (pollTimeoutRef.current !== null) {
        window.clearTimeout(pollTimeoutRef.current);
      }
    };
  }, [checkForUpdates]);

  const handleRemindLater = useCallback(() => {
    if (!servicePayload) return;
    writeReminder(
      servicePayload.latestVersion,
      postponeUntil(servicePayload.desktop.remindAfterHours)
    );
    setDialogOpen(false);
  }, [servicePayload]);

  const handleInstallNow = useCallback(async () => {
    if (!servicePayload || !versionStatus) return;

    if (!isDesktop || !servicePayload.desktop.enabled) {
      openExternal(servicePayload.web.releasePageUrl);
      setDialogOpen(false);
      return;
    }

    setInstallError(null);
    setDownloadedBytes(0);
    setContentLength(0);

    try {
      if (servicePayload.desktop.backupRequired) {
        setInstallPhase("backing-up");
        const backup = await tauriCommands.prepareUpdateBackup();
        setBackupPath(backup.backupPath);
      }

      setInstallPhase("checking");
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check({
        timeout: 15000,
        headers: {
          "X-CheersAI-Channel": servicePayload.channel,
        },
      });

      if (!update || compareVersions(update.version, currentVersion) <= 0) {
        throw new Error("更新清单尚未发布或当前已是最新版本。");
      }

      setInstallPhase("downloading");
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            setContentLength(event.data.contentLength ?? 0);
            setDownloadedBytes(0);
            break;
          case "Progress":
            setDownloadedBytes((current) => current + event.data.chunkLength);
            break;
          case "Finished":
            break;
        }
      });

      clearReminder();
      setInstallPhase("ready-to-restart");
    } catch (error) {
      setInstallPhase("error");
      setInstallError(error instanceof Error ? error.message : "更新失败，请稍后重试。");
    }
  }, [currentVersion, isDesktop, servicePayload, versionStatus]);

  const handleRestart = useCallback(async () => {
    await tauriCommands.restartApp();
  }, []);

  if (!servicePayload || !versionStatus || !versionStatus.updateAvailable) {
    return null;
  }

  const latestDisplay = formatDisplayVersion(servicePayload.latestVersion);
  const currentDisplay = formatDisplayVersion(currentVersion);
  const downloadPercent =
    contentLength > 0 ? Math.min(100, Math.round((downloadedBytes / contentLength) * 100)) : 0;
  const desktopCtaLabel = !isDesktop
    ? "查看发布说明"
    : installPhase === "ready-to-restart"
      ? "立即重启应用"
      : "立即更新";

  return (
    <Dialog
      open={dialogOpen}
      onOpenChange={(open) => {
        if (versionStatus.forceUpdate) return;
        setDialogOpen(open);
      }}
    >
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <RefreshCw className="h-5 w-5 text-blue-600" />
            检测到新版本 {latestDisplay}
          </DialogTitle>
          <DialogDescription>
            当前运行版本为 {currentDisplay}，已发现可用的新版本更新。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="flex flex-wrap gap-2">
            <Badge variant="info">{servicePayload.channel}</Badge>
            <Badge variant={versionStatus.forceUpdate ? "warning" : "success"}>
              {versionStatus.forceUpdate ? "强制升级" : "可选升级"}
            </Badge>
            <Badge variant="neutral">{versionStatus.releaseLevel.toUpperCase()}</Badge>
          </div>

          {servicePayload.releaseNotesSummary && (
            <p className="text-sm text-slate-700">{servicePayload.releaseNotesSummary}</p>
          )}

          {servicePayload.releaseNotes.length > 0 && (
            <ul className="space-y-2 rounded-lg border border-slate-200 bg-slate-50 p-4 text-sm text-slate-700">
              {servicePayload.releaseNotes.map((note, index) => (
                <li key={`${note.kind}-${index}`} className="flex gap-2">
                  <span className="mt-1 h-1.5 w-1.5 rounded-full bg-slate-400" />
                  <span>{note.text}</span>
                </li>
              ))}
            </ul>
          )}

          {servicePayload.desktop.backupRequired && isDesktop && (
            <Message type="info" title="更新前自动备份">
              安装前会先备份应用数据目录，避免升级过程影响已有数据。
            </Message>
          )}

          {backupPath && (
            <Message type="success" title="备份已完成">
              备份目录：{backupPath}
            </Message>
          )}

          {installPhase === "downloading" && (
            <div className="rounded-lg border border-slate-200 p-4">
              <div className="mb-2 flex items-center justify-between text-sm text-slate-700">
                <span>后台静默下载中</span>
                <span>{downloadPercent}%</span>
              </div>
              <div className="h-2 overflow-hidden rounded-full bg-slate-100">
                <div
                  className="h-full rounded-full bg-blue-600 transition-all"
                  style={{ width: `${downloadPercent}%` }}
                />
              </div>
            </div>
          )}

          {installPhase === "ready-to-restart" && (
            <Message type="success" title="更新已安装">
              新版本已经下载并安装完成，重启应用后即可生效。
            </Message>
          )}

          {installError && (
            <Message type="error" title="更新失败">
              {installError}
            </Message>
          )}

          {versionStatus.forceUpdate && (
            <Message type="warning" title="兼容性提示">
              当前版本已低于最低受支持版本，建议尽快升级以避免兼容性问题。
            </Message>
          )}
        </div>

        <DialogFooter className="gap-2">
          <Button
            variant="outline"
            onClick={() =>
              openExternal(isDesktop ? servicePayload.desktop.releasePageUrl : servicePayload.web.releasePageUrl)
            }
          >
            <ExternalLink className="h-4 w-4" />
            查看版本说明
          </Button>
          {!versionStatus.forceUpdate && (
            <Button variant="outline" onClick={handleRemindLater}>
              稍后提醒
            </Button>
          )}
          <Button
            onClick={() =>
              void (installPhase === "ready-to-restart" ? handleRestart() : handleInstallNow())
            }
            disabled={installPhase === "backing-up" || installPhase === "checking" || installPhase === "downloading"}
          >
            {installPhase === "backing-up" && <ShieldCheck className="h-4 w-4" />}
            {installPhase === "downloading" && <Download className="h-4 w-4" />}
            {desktopCtaLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
