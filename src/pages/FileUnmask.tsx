import { useState } from "react";
import { Upload, FileText, Key, Download, AlertCircle, CheckCircle2 } from "lucide-react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { tauriCommands } from "@/lib/tauri";

export default function FileUnmask() {
  const [maskedFile, setMaskedFile] = useState<string>("");
  const [mappingFile, setMappingFile] = useState<string>("");
  const [passphrase, setPassphrase] = useState<string>("");
  const [processing, setProcessing] = useState(false);
  const [result, setResult] = useState<{ output_path: string; restored_count: number } | null>(null);
  const [error, setError] = useState<string>("");

  const selectMaskedFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "支持的文件", extensions: ["txt", "md", "csv"] },
        { name: "所有文件", extensions: ["*"] },
      ],
    });
    if (selected) {
      setMaskedFile(selected as string);
      setError("");
      setResult(null);
    }
  };

  const selectMappingFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [
        { name: "对照文件", extensions: ["cmap"] },
        { name: "所有文件", extensions: ["*"] },
      ],
    });
    if (selected) {
      setMappingFile(selected as string);
      setError("");
      setResult(null);
    }
  };

  const handleUnmask = async () => {
    if (!maskedFile || !mappingFile || !passphrase) {
      setError("请选择文件并输入解密口令");
      return;
    }

    setProcessing(true);
    setError("");
    setResult(null);

    try {
      // 选择输出路径
      const outputPath = await save({
        defaultPath: maskedFile.replace(/\.(txt|md|csv)$/, "_restored.$1"),
        filters: [
          { name: "文本文件", extensions: ["txt", "md", "csv"] },
        ],
      });

      if (!outputPath) {
        setProcessing(false);
        return;
      }

      // 调用反脱敏命令
      const unmaskResult = await tauriCommands.unmaskFile({
        masked_file_path: maskedFile,
        mapping_file_path: mappingFile,
        passphrase: passphrase,
        output_path: outputPath,
      });

      setResult(unmaskResult);
      setPassphrase(""); // 清空口令
    } catch (err) {
      const errorMsg = err as string;
      // 优化错误提示
      if (errorMsg.includes("wrong passphrase") || errorMsg.includes("Decryption failed")) {
        setError("解密失败：加密口令不正确，请确认您输入的口令与创建对照文件时使用的口令一致");
      } else if (errorMsg.includes("Invalid magic bytes") || errorMsg.includes("Data too short")) {
        setError("对照文件格式错误或已损坏，请确认选择了正确的 .cmap 文件");
      } else {
        setError(errorMsg);
      }
    } finally {
      setProcessing(false);
    }
  };

  return (
    <div className="h-full flex flex-col bg-[#f9fafb]">
      <div className="flex-1 overflow-auto p-8">
        <div className="max-w-3xl mx-auto">
          <div className="mb-8">
            <h1 className="text-2xl font-bold text-[#111827] mb-2">文件反脱敏</h1>
            <p className="text-sm text-[#6b7280]">上传已脱敏的文件和对照文件，使用解密口令还原原始内容</p>
          </div>

          {/* 已脱敏文件选择 */}
          <div className="bg-white rounded-lg border border-[#e5e7eb] p-5 mb-6 shadow-sm">
            <div className="flex items-center gap-3 mb-4">
              <FileText className="w-5 h-5 text-[#3b82f6]" />
              <h2 className="text-base font-semibold text-[#111827]">已脱敏文件</h2>
            </div>
            <button
              onClick={selectMaskedFile}
              className="w-full border-2 border-dashed border-[#cbd5e1] rounded-lg p-6 hover:border-[#3b82f6] hover:bg-[#3b82f6]/5 transition-all"
              style={{ transitionDuration: '200ms' }}
            >
              <Upload className="w-8 h-8 mx-auto mb-2 text-[#9ca3af]" />
              <p className="text-sm text-[#6b7280]">
                {maskedFile ? maskedFile.split(/[\\/]/).pop() : "点击选择已脱敏的文件"}
              </p>
            </button>
          </div>

          {/* 对照文件选择 */}
          <div className="bg-white rounded-lg border border-[#e5e7eb] p-5 mb-6 shadow-sm">
            <div className="flex items-center gap-3 mb-4">
              <FileText className="w-5 h-5 text-[#8b5cf6]" />
              <h2 className="text-base font-semibold text-[#111827]">对照文件 (.cmap)</h2>
            </div>
            <button
              onClick={selectMappingFile}
              className="w-full border-2 border-dashed border-[#cbd5e1] rounded-lg p-6 hover:border-[#8b5cf6] hover:bg-[#8b5cf6]/5 transition-all"
              style={{ transitionDuration: '200ms' }}
            >
              <Upload className="w-8 h-8 mx-auto mb-2 text-[#9ca3af]" />
              <p className="text-sm text-[#6b7280]">
                {mappingFile ? mappingFile.split(/[\\/]/).pop() : "点击选择对照文件"}
              </p>
            </button>
          </div>

          {/* 解密口令输入 */}
          <div className="bg-white rounded-lg border border-[#e5e7eb] p-5 mb-6 shadow-sm">
            <div className="flex items-center gap-3 mb-4">
              <Key className="w-5 h-5 text-[#10b981]" />
              <h2 className="text-base font-semibold text-[#111827]">解密口令</h2>
            </div>
            <input
              type="password"
              value={passphrase}
              onChange={(e) => setPassphrase(e.target.value)}
              placeholder="输入加密口令以解密对照文件"
              className="w-full px-4 py-2.5 text-sm border border-[#d1d5db] rounded-lg text-[#111827] focus:outline-none focus:border-[#3b82f6] focus:ring-2 focus:ring-[#3b82f6]/10 transition-all"
              style={{ transitionDuration: '200ms' }}
            />
          </div>

          {/* 错误提示 */}
          {error && (
            <div className="bg-red-50 border border-red-200 rounded-lg p-4 mb-6 flex items-start gap-3">
              <AlertCircle className="w-5 h-5 text-red-600 flex-shrink-0 mt-0.5" />
              <div>
                <p className="text-sm font-medium text-red-900">反脱敏失败</p>
                <p className="text-sm text-red-700 mt-1">{error}</p>
              </div>
            </div>
          )}

          {/* 成功提示 */}
          {result && (
            <div className="bg-green-50 border border-green-200 rounded-lg p-4 mb-6 flex items-start gap-3">
              <CheckCircle2 className="w-5 h-5 text-green-600 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="text-sm font-medium text-green-900">反脱敏成功</p>
                <p className="text-sm text-green-700 mt-1">
                  已还原 {result.restored_count} 处敏感信息
                </p>
                <p className="text-xs text-green-600 mt-2 break-all">
                  输出文件：{result.output_path}
                </p>
              </div>
            </div>
          )}

          {/* 开始反脱敏按钮 */}
          <button
            onClick={handleUnmask}
            disabled={!maskedFile || !mappingFile || !passphrase || processing}
            className="w-full h-12 bg-[#3b82f6] text-white rounded-lg text-sm font-medium hover:bg-[#2563eb] disabled:opacity-50 disabled:cursor-not-allowed transition-all shadow-md hover:shadow-lg flex items-center justify-center gap-2"
            style={{ transitionDuration: '200ms' }}
          >
            {processing ? (
              <>
                <div className="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
                <span>正在还原...</span>
              </>
            ) : (
              <>
                <Download className="w-5 h-5" />
                <span>开始反脱敏</span>
              </>
            )}
          </button>

          {/* 使用说明 */}
          <div className="mt-8 bg-blue-50 border border-blue-200 rounded-lg p-6">
            <h3 className="text-sm font-semibold text-blue-900 mb-3">使用说明</h3>
            <ol className="text-sm text-blue-800 space-y-2 list-decimal list-inside">
              <li>选择需要还原的已脱敏文件（支持 TXT、MD、CSV 格式）</li>
              <li>选择对应的对照文件（.cmap 文件）</li>
              <li>输入创建对照文件时使用的加密口令</li>
              <li>点击"开始反脱敏"按钮，选择输出路径</li>
              <li>系统将自动解密对照文件并还原原始内容</li>
            </ol>
          </div>
        </div>
      </div>
    </div>
  );
}
