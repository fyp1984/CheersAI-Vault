/* Copyright 2026 CheersAI Team. */
import { useCallback, useEffect, useState } from "react";
import { UploadCloud } from "lucide-react";
import { cn } from "@/lib/utils";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useExcelMaskingStore } from "@/store/excelMaskingStore";
import { useFileStore } from "@/store/fileStore";
import { tauriCommands } from "@/lib/tauri";
import ExcelMaskingDialog from "@/components/file/ExcelMaskingDialog";
import type { ExcelApplyResult, ExcelMaskingConfig } from "@/types/commands";

interface DropZoneProps {
  onFilesDropped: (paths: string[]) => void;
}

function isExcelFile(p: string): boolean {
  const lower = p.toLowerCase();
  return (
    lower.endsWith(".xlsx") ||
    lower.endsWith(".xls") ||
    lower.endsWith(".xlsm") ||
    lower.endsWith(".csv")
  );
}

export function DropZone({ onFilesDropped }: DropZoneProps) {
  const [isDragActive, setIsDragActive] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [pendingPaths, setPendingPaths] = useState<string[]>([]);
  const { privacy } = useExcelMaskingStore();
  const { outputDir, passphrase } = useFileStore();

  const dispatchPaths = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) return;
      const excelPaths = paths.filter(isExcelFile);
      const hasExcel = excelPaths.length > 0;
      if (privacy.excelAutoMaskDialog && hasExcel) {
        setPendingPaths(paths);
        setDialogOpen(true);
        return;
      }
      onFilesDropped(paths);
    },
    [privacy.excelAutoMaskDialog, onFilesDropped]
  );

  const handleFileSelect = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: "支持的文件",
            extensions: [
              "csv",
              "xlsx",
              "xls",
              "json",
              "txt",
              "docx",
              "doc",
              "pptx",
              "ppt",
              "pdf",
              "md",
              "markdown",
            ],
          },
        ],
      });

      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        console.log("Selected files via dialog:", paths);
        dispatchPaths(paths);
      }
    } catch (error) {
      console.error("Failed to open file dialog:", error);
    }
  };

  const handleDialogCancel = useCallback(() => {
    const allPaths = pendingPaths;
    setPendingPaths([]);
    setDialogOpen(false);
    onFilesDropped(allPaths);
  }, [pendingPaths, onFilesDropped]);

  const handleDialogConfirm = useCallback(
    async (
      configs: ExcelMaskingConfig[],
      outputDirOverride?: string
    ) => {
      const outDir = outputDirOverride || outputDir;
      const nonExcel = pendingPaths.filter((p) => !isExcelFile(p));

      let producedPaths: string[] = [];
      for (const cfg of configs) {
        try {
          const res: ExcelApplyResult = await tauriCommands.excelApplyMasking(
            cfg,
            outDir
          );
          const candidates = [
            res.masked_path,
            res.ecmap_path,
            res.encrypted_source_path,
          ].filter((s): s is string => Boolean(s));
          producedPaths = producedPaths.concat(candidates);
        } catch (err) {
          console.error("excelApplyMasking failed:", cfg.file_path, err);
          producedPaths.push(cfg.file_path);
        }
      }

      const originalExcel = pendingPaths.filter(isExcelFile);
      const fallbackExcel = producedPaths.length > 0 ? [] : originalExcel;
      const finalPaths = [
        ...nonExcel,
        ...producedPaths,
        ...fallbackExcel,
      ];
      setPendingPaths([]);
      setDialogOpen(false);
      onFilesDropped(finalPaths);
    },
    [pendingPaths, outputDir, onFilesDropped]
  );

  useEffect(() => {
    let unlistenEnter: (() => void) | null = null;
    let unlistenOver: (() => void) | null = null;
    let unlistenDrop: (() => void) | null = null;
    let unlistenLeave: (() => void) | null = null;

    listen<string[]>("tauri://drag-enter", () => {
      setIsDragActive(true);
    })
      .then((fn) => {
        unlistenEnter = fn;
      })
      .catch((err) => {
        console.error("Failed to register tauri://drag-enter:", err);
      });

    listen<string[]>("tauri://drag-over", () => {
      setIsDragActive(true);
    })
      .then((fn) => {
        unlistenOver = fn;
      })
      .catch((err) => {
        console.error("Failed to register tauri://drag-over:", err);
      });

    listen<{ paths: string[]; position: { x: number; y: number } }>(
      "tauri://drag-drop",
      (event) => {
        setIsDragActive(false);

        if (event.payload.paths && event.payload.paths.length > 0) {
          dispatchPaths(event.payload.paths);
        }
      }
    )
      .then((fn) => {
        unlistenDrop = fn;
      })
      .catch((err) => {
        console.error("Failed to register tauri://drag-drop:", err);
      });

    listen("tauri://drag-leave", () => {
      setIsDragActive(false);
    })
      .then((fn) => {
        unlistenLeave = fn;
      })
      .catch((err) => {
        console.error("Failed to register tauri://drag-leave:", err);
      });

    return () => {
      if (unlistenEnter) unlistenEnter();
      if (unlistenOver) unlistenOver();
      if (unlistenDrop) unlistenDrop();
      if (unlistenLeave) unlistenLeave();
    };
  }, [dispatchPaths]);

  return (
    <>
      <div
        className={cn(
          "flex flex-col items-center justify-center w-full h-48 border-2 border-dashed rounded-xl cursor-pointer transition-colors",
          isDragActive
            ? "border-indigo-400 bg-indigo-50"
            : "border-gray-200 bg-gray-50 hover:border-indigo-300 hover:bg-indigo-50/50"
        )}
        onClick={handleFileSelect}
      >
        <UploadCloud
          className={cn(
            "w-10 h-10 mb-3 transition-colors",
            isDragActive ? "text-indigo-500" : "text-gray-400"
          )}
        />
        <p className="text-sm font-medium text-gray-600">点击选择文件</p>
        <p className="mt-1 text-xs text-gray-400">
          支持 CSV、Excel、JSON、TXT、Word、PowerPoint、PDF、Markdown
        </p>
        <p className="mt-0.5 text-xs text-gray-400">
          注：脱敏结果统一保存为 Markdown（.md）格式
        </p>
      </div>

      <ExcelMaskingDialog
        open={dialogOpen}
        onOpenChange={(o) => {
          if (!o) {
            setPendingPaths([]);
          }
          setDialogOpen(o);
        }}
        filePaths={pendingPaths}
        onCancel={handleDialogCancel}
        onConfirm={handleDialogConfirm}
        defaultPassphrase={passphrase}
        defaultOutputDir={outputDir}
      />
    </>
  );
}
