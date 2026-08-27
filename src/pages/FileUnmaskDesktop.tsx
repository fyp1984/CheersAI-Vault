/* Copyright 2026 CheersAI Team. */
import { useMemo, useState } from "react";
import {
  Upload,
  FileText,
  Key,
  Download,
  AlertCircle,
  CheckCircle2,
  ShieldCheck,
  FolderKanban,
  FileKey2,
  Files,
} from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { tauriCommands } from "@/lib/tauri";
import { useUnmaskStore } from "@/store/unmaskStore";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import { Message } from "@/components/ui/cheersai-ui";
import { RETAIN_MESSAGES } from "@/components/file/ExcelMaskingDialog";
import type { ExcelRestoreResult } from "@/types/commands";

// The Rust command `excel_restore_from_ecmap` returns
// `ExcelRestoreResult { restored_path: String, sha256_verified: bool }` — it
// never had a `restored_count` or `matched` field. The success card
// previously read those nonexistent fields anyway, so it always showed an
// empty restored count and, since `!undefined` is `true`, always claimed
// "SHA 未匹配" even on a fully verified restore. This is the single place
// that turns a real `ExcelRestoreResult` into display text, so a targeted
// test can pin the exact fields consumed and catch either regression.
export function describeExcelRestoreSuccess(result: ExcelRestoreResult): {
  statusText: string;
  outputPath: string;
} {
  return {
    statusText: result.sha256_verified ? "SHA-256 校验通过" : "SHA-256 未通过校验",
    outputPath: result.restored_path,
  };
}

export default function FileUnmaskDesktop() {
  const { maskedFile, mappingFile, setMaskedFile, setMappingFile } =
    useUnmaskStore();

  const [restoreMode, setRestoreMode] = useState<"A" | "B">("A");
  const [passphrase, setPassphrase] = useState<string>("");
  const [processing, setProcessing] = useState(false);
  const [result, setResult] = useState<ExcelRestoreResult | null>(null);
  const [legacyResult, setLegacyResult] = useState<{
    output_path: string;
    restored_count: number;
  } | null>(null);
  const [error, setError] = useState<string>("");

  const [encryptedSource, setEncryptedSource] = useState<string>("");
  const [userOriginalFile, setUserOriginalFile] = useState<string>("");

  const excelRestoreDisplay = useMemo(
    () => (result ? describeExcelRestoreSuccess(result) : null),
    [result]
  );

  const missingA = useMemo(() => {
    return !maskedFile || !mappingFile || !encryptedSource;
  }, [maskedFile, mappingFile, encryptedSource]);

  const missingB = useMemo(() => {
    return !maskedFile || !mappingFile || !userOriginalFile;
  }, [maskedFile, mappingFile, userOriginalFile]);

  const isExcelFlow = useMemo(() => {
    const lower = (mappingFile || "").toLowerCase();
    return lower.endsWith(".ecmap");
  }, [mappingFile]);

  const selectMaskedFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "支持的文件",
          extensions: ["txt", "md", "csv", "xlsx", "docx", "pdf", "pptx"],
        },
        { name: "所有文件", extensions: ["*"] },
      ],
    });
    if (selected) {
      setMaskedFile(selected as string);
      setError("");
      setResult(null);
      setLegacyResult(null);
    }
  };

  const selectMappingFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "Excel 映射对照", extensions: ["ecmap"] },
        { name: "旧版对照文件", extensions: ["cmap"] },
        { name: "所有文件", extensions: ["*"] },
      ],
    });
    if (selected) {
      setMappingFile(selected as string);
      setError("");
      setResult(null);
      setLegacyResult(null);
    }
  };

  const selectEncryptedSource = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "加密源文件", extensions: ["encrypted_src"] },
        { name: "所有文件", extensions: ["*"] },
      ],
    });
    if (selected) {
      setEncryptedSource(selected as string);
      setError("");
      setResult(null);
    }
  };

  const selectUserOriginalFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        {
          name: "用户原件 xlsx/csv",
          extensions: ["xlsx", "xls", "xlsm", "csv"],
        },
        { name: "所有文件", extensions: ["*"] },
      ],
    });
    if (selected) {
      setUserOriginalFile(selected as string);
      setError("");
      setResult(null);
    }
  };

  const handleUnmask = async () => {
    if (isExcelFlow) {
      if (restoreMode === "A" && missingA) {
        setError(RETAIN_MESSAGES.unmask_missing);
        return;
      }
      if (restoreMode === "B" && missingB) {
        setError(RETAIN_MESSAGES.unmask_missing);
        return;
      }
    } else {
      if (!maskedFile || !mappingFile || !passphrase) {
        setError("请选择文件并输入解密口令");
        return;
      }
    }

    setProcessing(true);
    setError("");
    setResult(null);
    setLegacyResult(null);

    try {
      const originalFileName =
        maskedFile.split(/[\\/]/).pop() || "file";
      const fileNameWithoutExt = originalFileName.replace(/\.[^.]+$/, "");
      const fileExt = originalFileName.match(/\.[^.]+$/)?.[0] || ".txt";
      const defaultFileName = `${fileNameWithoutExt}_已还原${fileExt}`;
      const defaultDir = maskedFile.substring(
        0,
        maskedFile.lastIndexOf(
          /[\\/]/.test(maskedFile)
            ? maskedFile.match(/[\\/][^\\/]*$/)?.[0] || ""
            : ""
        )
      );
      const defaultPath = defaultDir
        ? `${defaultDir}/${defaultFileName}`
        : defaultFileName;

      const outputPath = await save({
        defaultPath: defaultPath,
        filters: [
          {
            name: "支持的文件",
            extensions: ["txt", "md", "csv", "xlsx", "docx", "pdf", "pptx"],
          },
        ],
      });

      if (!outputPath) {
        setProcessing(false);
        return;
      }

      if (isExcelFlow) {
        const res: ExcelRestoreResult =
          await tauriCommands.excelRestoreFromEcmap({
            restore_mode: restoreMode,
            masked_file_path: maskedFile,
            ecmap_file_path: mappingFile,
            encrypted_source_path:
              restoreMode === "A" ? encryptedSource : undefined,
            user_original_file_path:
              restoreMode === "B" ? userOriginalFile : undefined,
            passphrase: passphrase || undefined,
            output_path: outputPath,
          });
        setResult(res);
        setPassphrase("");
      } else {
        const unmaskResult = await tauriCommands.unmaskFile({
          masked_file_path: maskedFile,
          mapping_file_path: mappingFile,
          passphrase: passphrase,
          output_path: outputPath,
        });
        setLegacyResult(unmaskResult);
        setPassphrase("");
      }
    } catch (err) {
      const errorMsg = err as string;
      if (
        errorMsg.includes("wrong passphrase") ||
        errorMsg.includes("Decryption failed")
      ) {
        setError(
          "解密失败：加密口令不正确，请确认您输入的口令与创建对照文件时使用的口令一致"
        );
      } else if (
        errorMsg.includes("Invalid magic bytes") ||
        errorMsg.includes("Data too short")
      ) {
        setError("对照文件格式错误或已损坏，请确认选择了正确的对照文件");
      } else {
        setError(errorMsg);
      }
    } finally {
      setProcessing(false);
    }
  };

  return (
    <div className="h-full flex flex-col bg-[#f9fafb]">
      <div className="flex-1 overflow-auto p-6">
        <div className="w-full max-w-6xl mx-auto">
          <div className="mb-8">
            <h1 className="text-2xl font-bold text-[#111827] mb-2">
              文件反脱敏
            </h1>
            <p className="text-sm text-[#6b7280]">
              上传已脱敏的文件和对照文件，使用解密口令还原原始内容
            </p>
          </div>

          {isExcelFlow ? (
            <Tabs
              value={restoreMode}
              onValueChange={(v) =>
                setRestoreMode(v as "A" | "B")
              }
            >
              <TabsList className="mb-6">
                <TabsTrigger value="A">
                  <FileKey2 className="w-4 h-4 mr-1.5" />
                  路径 A：加密源 + ecmap
                </TabsTrigger>
                <TabsTrigger value="B">
                  <Files className="w-4 h-4 mr-1.5" />
                  路径 B：用户原件匹配
                </TabsTrigger>
              </TabsList>

              <TabsContent value="A">
                <div className="grid grid-cols-1 xl:grid-cols-3 gap-6 mb-6">
                  <FilePickerCard
                    title="已脱敏 Excel"
                    icon={FileText}
                    iconColor="text-[#3b82f6]"
                    value={maskedFile}
                    onSelect={selectMaskedFile}
                    hint="点击选择已脱敏的 xlsx/csv 文件"
                  />
                  <FilePickerCard
                    title="映射对照 (.ecmap)"
                    icon={FolderKanban}
                    iconColor="text-blue-600"
                    value={mappingFile}
                    onSelect={selectMappingFile}
                    hint="点击选择 .ecmap 映射对照文件"
                  />
                  <FilePickerCard
                    title="加密源 (.encrypted_src)"
                    icon={ShieldCheck}
                    iconColor="text-purple-600"
                    value={encryptedSource}
                    onSelect={selectEncryptedSource}
                    hint="脱敏时勾选保留的加密源文件"
                  />
                </div>
                {missingA && (
                  <Message type="warning" title="缺少反脱敏材料">
                    <p className="text-xs leading-6">
                      {RETAIN_MESSAGES.unmask_missing}
                    </p>
                  </Message>
                )}
              </TabsContent>

              <TabsContent value="B">
                <div className="grid grid-cols-1 xl:grid-cols-3 gap-6 mb-6">
                  <FilePickerCard
                    title="已脱敏 Excel"
                    icon={FileText}
                    iconColor="text-[#3b82f6]"
                    value={maskedFile}
                    onSelect={selectMaskedFile}
                    hint="点击选择已脱敏的 xlsx/csv 文件"
                  />
                  <FilePickerCard
                    title="映射对照 (.ecmap)"
                    icon={FolderKanban}
                    iconColor="text-blue-600"
                    value={mappingFile}
                    onSelect={selectMappingFile}
                    hint="点击选择 .ecmap 映射对照文件"
                  />
                  <FilePickerCard
                    title="用户原件 (路径 B)"
                    icon={Files}
                    iconColor="text-amber-600"
                    value={userOriginalFile}
                    onSelect={selectUserOriginalFile}
                    hint="原件 sha 需与 ecmap.header 完全一致"
                  />
                </div>
                <div className="bg-blue-50 border border-blue-200 rounded-lg p-4 mb-6 flex items-start gap-3">
                  <ShieldCheck className="w-5 h-5 text-blue-600 flex-shrink-0 mt-0.5" />
                  <div>
                    <p className="text-sm font-medium text-blue-900">
                      路径 B 说明
                    </p>
                    <p className="text-xs text-blue-700 mt-1 leading-6">
                      需要用户原件 sha256 与 ecmap.header 内记录的原件指纹完全一致；
                      若不勾选「保留加密源」，这是唯一可行的自动反脱敏路径。
                    </p>
                  </div>
                </div>
              </TabsContent>
            </Tabs>
          ) : (
            <div className="grid grid-cols-1 xl:grid-cols-2 gap-6 mb-6">
              <FilePickerCard
                title="已脱敏文件"
                icon={FileText}
                iconColor="text-[#3b82f6]"
                value={maskedFile}
                onSelect={selectMaskedFile}
                hint="点击选择已脱敏的文件"
              />
              <FilePickerCard
                title="对照文件 (.cmap)"
                icon={FileText}
                iconColor="text-[#3b82f6]"
                value={mappingFile}
                onSelect={selectMappingFile}
                hint="点击选择对照文件"
              />
            </div>
          )}

          <div className="bg-white rounded-lg border border-[#e5e7eb] p-5 mb-6 shadow-sm">
            <div className="flex items-center gap-3 mb-4">
              <Key className="w-5 h-5 text-[#10b981]" />
              <h2 className="text-base font-semibold text-[#111827]">
                解密口令（可选）
              </h2>
            </div>
            <input
              type="password"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              placeholder="输入加密口令以解密对照文件（SECONDARY_PASSPHRASE 模式才需要）"
              className="w-full px-4 py-2.5 text-sm border border-[#d1d5db] rounded-lg text-[#111827] focus:outline-none focus:border-[#3b82f6] focus:ring-2 focus:ring-[#3b82f6]/10 transition-all"
            />
          </div>

          {error && (
            <div className="bg-red-50 border border-red-200 rounded-lg p-4 mb-6 flex items-start gap-3">
              <AlertCircle className="w-5 h-5 text-red-600 flex-shrink-0 mt-0.5" />
              <div>
                <p className="text-sm font-medium text-red-900">
                  反脱敏失败
                </p>
                <p className="text-sm text-red-700 mt-1">{error}</p>
              </div>
            </div>
          )}

          {excelRestoreDisplay && (
            <div className="bg-emerald-50 border border-emerald-200 rounded-lg p-4 mb-6 flex items-start gap-3">
              <CheckCircle2 className="w-5 h-5 text-emerald-600 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="text-sm font-medium text-emerald-900">
                  反脱敏成功
                </p>
                <p className="text-sm text-emerald-700 mt-1">
                  {excelRestoreDisplay.statusText}
                </p>
                <p className="text-xs text-emerald-600 mt-2 break-all">
                  输出文件：{excelRestoreDisplay.outputPath}
                </p>
              </div>
            </div>
          )}

          {legacyResult && (
            <div className="bg-blue-50 border border-blue-200 rounded-lg p-4 mb-6 flex items-start gap-3">
              <CheckCircle2 className="w-5 h-5 text-blue-600 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="text-sm font-medium text-green-900">
                  反脱敏成功
                </p>
                <p className="text-sm text-blue-700 mt-1">
                  已还原 {legacyResult.restored_count} 处敏感信息
                </p>
                <p className="text-xs text-blue-600 mt-2 break-all">
                  输出文件：{legacyResult.output_path}
                </p>
              </div>
            </div>
          )}

          <button
            onClick={handleUnmask}
            disabled={
              processing ||
              (isExcelFlow
                ? restoreMode === "A"
                  ? missingA
                  : missingB
                : !maskedFile || !mappingFile || !passphrase)
            }
            className="w-full h-12 bg-[#3b82f6] text-white rounded-lg text-sm font-medium hover:bg-[#2563eb] disabled:opacity-50 disabled:cursor-not-allowed transition-all shadow-md hover:shadow-lg flex items-center justify-center gap-2"
          >
            {processing ? (
              <>
                <div className="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin" />
                <span>正在还原...</span>
              </>
            ) : (
              <>
                <Download className="w-5 h-5" />
                <span>开始反脱敏</span>
              </>
            )}
          </button>

          <div className="mt-8 bg-blue-50 border border-blue-200 rounded-lg p-6">
            <h3 className="text-sm font-semibold text-blue-900 mb-3">
              使用说明
            </h3>
            <ol className="text-sm text-blue-800 space-y-2 list-decimal list-inside">
              <li>选择需要还原的已脱敏文件</li>
              <li>
                选择映射对照文件（Excel 脱敏产物为 .ecmap，旧版为 .cmap）
              </li>
              <li>
                Excel 反脱敏二选一：路径 A 需要加密源文件（推荐）；路径 B 需要用户原件且 sha 与 ecmap 完全一致
              </li>
              <li>若密钥模式为 SECONDARY_PASSPHRASE，输入对应二级口令</li>
              <li>点击「开始反脱敏」选择输出路径</li>
            </ol>
          </div>
        </div>
      </div>
    </div>
  );
}

interface FilePickerCardProps {
  title: string;
  icon: React.ComponentType<{ className?: string }>;
  iconColor: string;
  value: string;
  onSelect: () => void;
  hint: string;
}

function FilePickerCard({
  title,
  icon: Icon,
  iconColor,
  value,
  onSelect,
  hint,
}: FilePickerCardProps) {
  return (
    <div className="bg-white rounded-lg border border-[#e5e7eb] p-5 shadow-sm">
      <div className="flex items-center gap-3 mb-4">
        <Icon className={`w-5 h-5 ${iconColor}`} />
        <h2 className="text-base font-semibold text-[#111827]">{title}</h2>
      </div>
      <button
        onClick={onSelect}
        className="w-full min-h-[132px] border-2 border-dashed border-[#cbd5e1] rounded-lg p-6 hover:border-[#3b82f6] hover:bg-[#3b82f6]/5 transition-all"
      >
        <Upload className="w-8 h-8 mx-auto mb-2 text-[#9ca3af]" />
        <p className="text-sm text-[#6b7280] break-all">
          {value ? value.split(/[\\/]/).pop() : hint}
        </p>
      </button>
    </div>
  );
}
