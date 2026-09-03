/* Copyright 2026 CheersAI Team. */
import { useCallback, useEffect, useState } from "react";
import { AlertTriangle, CheckCircle2, UploadCloud } from "lucide-react";
import { cn } from "@/lib/utils";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { useExcelMaskingStore } from "@/store/excelMaskingStore";
import { useFileStore } from "@/store/fileStore";
import { classifyDesktopExcelApplyError } from "@/lib/excelMaskingContract";
import { tauriCommands } from "@/lib/tauri";
import ExcelMaskingDialog from "@/components/file/ExcelMaskingDialog";
import type { ExcelApplyResult, ExcelMaskingConfig } from "@/types/commands";

interface DropZoneProps {
  onFilesDropped: (paths: string[]) => void;
}

export function isExcelFile(p: string): boolean {
  const lower = p.toLowerCase();
  return (
    lower.endsWith(".xlsx") ||
    lower.endsWith(".xls") ||
    lower.endsWith(".xlsm") ||
    lower.endsWith(".csv")
  );
}

export function isProtectedExcelArtifact(p: string): boolean {
  const lower = p.toLowerCase();
  return lower.endsWith(".ecmap") || lower.endsWith(".encrypted_src");
}

export function partitionDropPaths(paths: string[]): {
  acceptedPaths: string[];
  protectedArtifacts: string[];
} {
  return {
    acceptedPaths: paths.filter((path) => !isProtectedExcelArtifact(path)),
    protectedArtifacts: paths.filter(isProtectedExcelArtifact),
  };
}

export function regularInputsAfterExcelFlow(paths: string[]): string[] {
  return paths.filter(
    (path) => !isExcelFile(path) && !isProtectedExcelArtifact(path)
  );
}

export interface ExcelOutputSummary {
  maskedPath: string;
  ecmapPath: string;
  encryptedSourcePath?: string;
}

interface ExcelApplyRoutingOptions {
  configs: ExcelMaskingConfig[];
  pendingPaths: string[];
  outputDir: string;
  sandboxPassphrase: string;
  applyMasking: (
    config: ExcelMaskingConfig,
    outputDir: string,
    sandboxPassphrase?: string
  ) => Promise<ExcelApplyResult>;
}

export interface ExcelApplyRoutingResult {
  outputs: ExcelOutputSummary[];
  normalQueuePaths: string[];
  failureCount: number;
  /**
   * R-closeout (工作包 D): 第一个失败的安全分类文案。只包含固定安全文案，
   * 绝不含原始错误中的路径、口令、堆栈或密文；无失败时为 undefined。
   */
  firstErrorMessage?: string;
}

export function toExcelOutputSummary(result: ExcelApplyResult): ExcelOutputSummary {
  if (
    result.status === "ERROR" ||
    !result.masked_path ||
    !result.ecmap_path
  ) {
    throw new Error("Excel 脱敏未生成完整的工作簿和映射产物。");
  }
  return {
    maskedPath: result.masked_path,
    ecmapPath: result.ecmap_path,
    encryptedSourcePath: result.encrypted_source_path || undefined,
  };
}

export async function executeExcelApplyRouting({
  configs,
  pendingPaths,
  outputDir,
  sandboxPassphrase,
  applyMasking,
}: ExcelApplyRoutingOptions): Promise<ExcelApplyRoutingResult> {
  const outputs: ExcelOutputSummary[] = [];
  let failureCount = 0;
  let firstError: unknown = null;

  for (const config of configs) {
    try {
      const result = await applyMasking(config, outputDir, sandboxPassphrase);
      outputs.push(toExcelOutputSummary(result));
    } catch (error) {
      failureCount += 1;
      if (firstError === null) {
        firstError = error;
      }
    }
  }

  const excelInputCount = pendingPaths.filter(isExcelFile).length;
  if (configs.length < excelInputCount) {
    failureCount += excelInputCount - configs.length;
  }

  return {
    outputs,
    normalQueuePaths: regularInputsAfterExcelFlow(pendingPaths),
    failureCount,
    firstErrorMessage:
      firstError === null ? undefined : classifyDesktopExcelApplyError(firstError),
  };
}

export const PROTECTED_EXCEL_ARTIFACT_INPUT_ERROR =
  "安全提示：.ecmap 和 .encrypted_src 是 Excel 脱敏产物，不能作为新的脱敏输入；如需恢复，请前往文件反脱敏。";

export const EXCEL_APPLY_FAILURE_MESSAGE =
  "Excel 脱敏执行失败，原始 Excel 和半成品均未加入普通处理队列，请检查配置或本地文件权限后重试。";

// E: Excel/CSV inputs go through the enhanced flow and come out as .xlsx;
// every other format still comes out as Markdown. Must never claim a single
// uniform output format for all inputs.
export const DROPZONE_OUTPUT_FORMAT_NOTE =
  "注：Excel/CSV 进入增强流程后输出为脱敏后的 .xlsx；其他格式仍保存为 Markdown（.md）";

export function DropZone({ onFilesDropped }: DropZoneProps) {
  const [isDragActive, setIsDragActive] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [pendingPaths, setPendingPaths] = useState<string[]>([]);
  const [excelOutputs, setExcelOutputs] = useState<ExcelOutputSummary[]>([]);
  const [routeMessage, setRouteMessage] = useState<string | null>(null);
  const { privacy } = useExcelMaskingStore();
  const { outputDir, passphrase } = useFileStore();

  const dispatchPaths = useCallback(
    (paths: string[]) => {
      if (paths.length === 0) return;
      const { acceptedPaths, protectedArtifacts } = partitionDropPaths(paths);
      if (protectedArtifacts.length > 0) {
        setRouteMessage(PROTECTED_EXCEL_ARTIFACT_INPUT_ERROR);
      } else {
        setRouteMessage(null);
      }
      if (acceptedPaths.length === 0) return;

      setExcelOutputs([]);
      const excelPaths = acceptedPaths.filter(isExcelFile);
      const hasExcel = excelPaths.length > 0;
      if (privacy.excelAutoMaskDialog && hasExcel) {
        setPendingPaths(acceptedPaths);
        setDialogOpen(true);
        return;
      }
      onFilesDropped(acceptedPaths);
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
      // UI-STATE-002 (TASK-EXCEL-OUTPUT-RECOVERY-CONSISTENCY-CLOSEOUT-001):
      // 确认后立即关闭配置弹窗并清空待处理路径，避免用户停留在弹窗中等待
      // PBKDF2 加密等耗时步骤（产物已写出但弹窗仍停留、取消/Escape 不生效的
      // 根因是应用尚未返回）；应用在后台完成，结果由下方的状态更新呈现在
      // 页面主区。executeExcelApplyRouting 使用闭包捕获的 pendingPaths，
      // 不受此处提前清空影响。
      setDialogOpen(false);
      setPendingPaths([]);
      const routing = await executeExcelApplyRouting({
        configs,
        pendingPaths,
        outputDir: outDir,
        sandboxPassphrase: passphrase,
        applyMasking: tauriCommands.excelApplyMasking,
      });

      if (routing.failureCount > 0) {
        console.error(
          "Excel enhanced masking failed; no Excel input or output was routed to the normal queue."
        );
      }
      if (routing.normalQueuePaths.length > 0) {
        onFilesDropped(routing.normalQueuePaths);
      }
      setExcelOutputs(routing.outputs);
      setRouteMessage(
        routing.failureCount > 0
          ? routing.firstErrorMessage ?? EXCEL_APPLY_FAILURE_MESSAGE
          : null
      );
    },
    [pendingPaths, outputDir, onFilesDropped, passphrase]
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
        <p className="mt-0.5 text-xs text-gray-400">{DROPZONE_OUTPUT_FORMAT_NOTE}</p>
      </div>

      {routeMessage && (
        <div
          role="alert"
          className="mt-3 flex items-start gap-2 rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-900"
        >
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <span>{routeMessage}</span>
        </div>
      )}

      {excelOutputs.length > 0 && (
        <div
          role="status"
          className="mt-3 rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-950"
        >
          <div className="flex items-center gap-2 font-medium">
            <CheckCircle2 className="h-4 w-4" />
            Excel 脱敏成功，产物已生成，无需再次点击普通“开始处理”
          </div>
          <div className="mt-2 space-y-3">
            {excelOutputs.map((output) => (
              <div key={`${output.maskedPath}::${output.ecmapPath}`} className="space-y-1">
                <p className="break-all">
                  <span className="font-medium">脱敏工作簿：</span>
                  {output.maskedPath}
                </p>
                <p className="break-all">
                  <span className="font-medium">映射文件：</span>
                  {output.ecmapPath}
                </p>
                <p className="break-all">
                  <span className="font-medium">加密原件：</span>
                  {output.encryptedSourcePath ?? "未生成（可使用用户原件走 Path B）"}
                </p>
              </div>
            ))}
          </div>
        </div>
      )}

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
