import { PageHeader } from "@/components/layout/PageHeader";
import { DropZone } from "@/components/file/DropZone";
import { FileQueueItem } from "@/components/file/FileQueueItem";
import { BatchProgress } from "@/components/file/BatchProgress";
import { RuleSelector } from "@/components/file/RuleSelector";
import { PassphraseBox } from "@/components/common/PassphraseBox";
import { MaskingPreviewDialog, type ManualReplacement } from "@/components/file/MaskingPreviewDialog";
import { OcrDownloadDialog } from "@/components/file/OcrDownloadDialog";
import { Button } from "@/components/ui/button";
import { useFileStore } from "@/store/fileStore";
import { useLogStore } from "@/store/logStore";
import { useRuleStore } from "@/store/ruleStore";
import { Play, Trash2, FolderOpen } from "lucide-react";
import { v4 as uuidv4 } from "uuid";
import { stat, exists } from "@tauri-apps/plugin-fs";
import { open } from "@tauri-apps/plugin-dialog";
import { tauriCommands } from "@/lib/tauri";
import { useEffect, useState } from "react";
import { getDisplayPath, validatePath, getDefaultDocumentsPath } from "@/lib/path";
import type { BatchStatus, PreviewResult } from "@/types/commands";

export default function FileProcess() {
  console.log("FileProcess page loaded");
  
  const { 
    files, 
    passphrase, 
    outputDir,
    activeJobId,
    addFiles, 
    removeFile, 
    clearCompleted, 
    setPassphrase,
    setOutputDir,
    setActiveJob,
    updateFile
  } = useFileStore();

  const { addLog } = useLogStore();
  const { rules } = useRuleStore();

  const [batchStatus, setBatchStatus] = useState<BatchStatus | null>(null);
  const [selectedRules, setSelectedRules] = useState<string[]>(() => {
    // 初始化时使用所有启用的规则
    return rules.filter((r) => r.enabled).map((r) => r.id);
  });
  const [previewData, setPreviewData] = useState<Array<{ fileName: string; preview: PreviewResult }>>([]);
  const [showPreview, setShowPreview] = useState(false);
  const [isLoadingPreview, setIsLoadingPreview] = useState(false);
  const [showOcrDownload, setShowOcrDownload] = useState(false);
  const [useAiValidation, setUseAiValidation] = useState(false); // AI 验证开关
  const [aiDetectionAvailable, setAiDetectionAvailable] = useState(false); // AI 是否可用
  const [isCheckingAi, setIsCheckingAi] = useState(true); // 是否正在检查 AI
  const [isProcessing, setIsProcessing] = useState(false); // 是否正在处理（包括预览和脱敏）

  // 检查 AI 检测是否可用
  useEffect(() => {
    const checkAiAvailability = async () => {
      try {
        console.log("Checking AI detection availability...");
        const available = await tauriCommands.checkAiDetectionAvailable();
        console.log("AI detection available:", available);
        setAiDetectionAvailable(available);
        
        // 如果 AI 可用，默认打开开关
        if (available) {
          setUseAiValidation(true);
          console.log("AI detection enabled by default");
        }
      } catch (error) {
        console.error("Failed to check AI availability:", error);
        setAiDetectionAvailable(false);
      } finally {
        setIsCheckingAi(false);
      }
    };

    checkAiAvailability();
  }, []);

  // 检测是否是 OCR 相关错误
  const isOcrError = (error: unknown): boolean => {
    const errorStr = String(error).toLowerCase();
    return errorStr.includes('ocr') || 
           errorStr.includes('扫描版') || 
           errorStr.includes('python') ||
           errorStr.includes('easyocr');
  };

  // 轮询批处理状态
  useEffect(() => {
    if (!activeJobId) {
      setBatchStatus(null);
      return;
    }

    const interval = setInterval(async () => {
      try {
        const status = await tauriCommands.getBatchStatus(activeJobId);
        setBatchStatus(status);
        
        // 更新文件状态
        files.forEach((file, index) => {
          if (index < status.completed) {
            updateFile(file.id, { status: "completed" });
          } else if (index === status.completed && status.current_file) {
            updateFile(file.id, { status: "processing" });
          } else if (index < status.completed + status.failed) {
            updateFile(file.id, { 
              status: "failed", 
              error: status.error || "处理失败" 
            });
          }
        });

        // 如果任务完成，停止轮询并显示通知
        if (status.status === "Completed") {
          setActiveJob(null);
          setIsProcessing(false); // 完成时重置状态
          
          // 记录完成日志
          const completedCount = status.completed;
          const failedCount = status.failed;
          
          if (failedCount === 0) {
            await addLog("success", "批处理完成", `成功: ${completedCount}, 失败: ${failedCount}`, undefined, "batch_complete");
          } else {
            await addLog("warning", "批处理完成（部分失败）", `成功: ${completedCount}, 失败: ${failedCount}`, undefined, "batch_partial");
          }
          
          setTimeout(() => setBatchStatus(null), 3000);
        } else if (status.status === "Failed") {
          setActiveJob(null);
          setIsProcessing(false); // 失败时重置状态
          await addLog("error", "批处理失败", status.error || "未知错误", undefined, "batch_failed");
          setTimeout(() => setBatchStatus(null), 3000);
        } else if (status.status === "Cancelled") {
          setActiveJob(null);
          setIsProcessing(false); // 取消时重置状态
          alert("⏹️ 脱敏已取消");
          await addLog("info", "批处理已取消", undefined, undefined, "batch_cancelled");
          setTimeout(() => setBatchStatus(null), 3000);
        }
      } catch (error) {
        console.error("Failed to get batch status:", error);
        setActiveJob(null);
        setIsProcessing(false); // 错误时重置状态
        setBatchStatus(null);
      }
    }, 1000);

    return () => clearInterval(interval);
  }, [activeJobId, files, updateFile, setActiveJob, outputDir, addLog]);

  const handleDrop = async (paths: string[]) => {
    const queued = await Promise.all(
      paths.map(async (p) => {
        let size = 0;
        try {
          console.log(`Checking file: ${p}`);
          const fileExists = await exists(p);
          console.log(`File exists: ${fileExists}`);
          
          if (fileExists) {
            const info = await stat(p);
            size = info.size ?? 0;
            console.log(`File size: ${size} bytes`);
          } else {
            console.warn(`File does not exist: ${p}`);
            size = -1;
          }
        } catch (error) {
          console.error(`Failed to get file info for ${p}:`, error);
          size = -1;
        }
        
        const fileName = p.split(/[\\/]/).pop() ?? p;
        console.log(`Creating queued file: ${fileName} (${p})`);
        
        return {
          id: uuidv4(),
          name: fileName,
          path: p,
          size,
          status: "pending" as const,
          addedAt: Date.now(),
        };
      })
    );
    addFiles(queued);
  };

  const handleSelectOutputDir = async () => {
    try {
      const selected = await open({
        directory: true,
        title: "选择输出目录",
        defaultPath: outputDir || getDefaultDocumentsPath(),
      });
      if (selected) {
        const selectedPath = selected as string;
        const validation = validatePath(selectedPath);
        
        if (validation.valid) {
          setOutputDir(selectedPath);
        } else {
          alert(`路径无效: ${validation.error}`);
        }
      }
    } catch (error) {
      console.error("Failed to select output directory:", error);
    }
  };

  const handleStart = async () => {
    if (pendingCount === 0) return;
    if (!outputDir) {
      alert("请先选择输出目录");
      return;
    }
    if (selectedRules.length === 0) {
      alert("请至少选择一个脱敏规则");
      return;
    }

    // 加载所有待处理文件的预览数据
    const pendingFiles = files.filter(f => f.status === "pending");
    if (pendingFiles.length === 0) return;

    setIsLoadingPreview(true);
    setIsProcessing(true); // 开始处理流程
    try {
      const customRules = rules
        .filter((r) => !r.builtin && r.enabled && selectedRules.includes(r.id))
        .map((r) => ({
          id: r.id,
          name: r.name,
          pattern: r.pattern,
          replacement_template: r.replacement_template,
          use_counter: r.use_counter,
        }));

      const previews = await Promise.all(
        pendingFiles.map(async (file) => {
          const preview = await tauriCommands.previewMasking({
            file_path: file.path,
            rule_ids: selectedRules,
            custom_rules: customRules.length > 0 ? customRules : undefined,
            use_ai_validation: useAiValidation,
          });
          return {
            fileName: file.name,
            preview,
          };
        })
      );
      setPreviewData(previews);
      setIsProcessing(false); // 预览加载完成，关闭动画
      setShowPreview(true);
    } catch (error) {
      console.error("Failed to load preview:", error);
      
      // 检查是否是 OCR 错误
      if (isOcrError(error)) {
        setShowOcrDownload(true);
      } else {
        alert(`加载预览失败: ${error}`);
      }
      setIsProcessing(false); // 失败时重置状态
    } finally {
      setIsLoadingPreview(false);
    }
  };

  const pendingCount = files.filter((f) => f.status === "pending").length;

  const handlePreviewConfirm = async (_manualReplacements: ManualReplacement[]) => {
    setShowPreview(false);
    setIsProcessing(true); // 显示保存进度
    
    try {
      // 直接保存预览结果，不重新处理
      const pendingFiles = files.filter(f => f.status === "pending");
      
      for (let i = 0; i < pendingFiles.length; i++) {
        const file = pendingFiles[i];
        const filePreview = previewData[i];
        
        if (!filePreview) continue;
        
        try {
          updateFile(file.id, { status: "processing" });
          
          const result = await tauriCommands.savePreviewResult({
            file_path: file.path,
            output_dir: outputDir,
            masked_rows: filePreview.preview.masked_rows,
            headers: filePreview.preview.headers,
            passphrase: passphrase || undefined,
          });
          
          updateFile(file.id, { status: "completed" });
          await addLog("success", "文件脱敏完成", `输出: ${result.output_path}`, file.path, "mask_file");
        } catch (error) {
          console.error(`Failed to save ${file.name}:`, error);
          updateFile(file.id, { status: "failed", error: String(error) });
          await addLog("error", "文件脱敏失败", String(error), file.path, "mask_file");
        }
      }
      
      setIsProcessing(false);
      setPreviewData([]);
    } catch (error) {
      console.error("Failed to save preview results:", error);
      setIsProcessing(false);
      await addLog("error", "保存预览结果失败", String(error), undefined, "batch_error");
    }
  };

  const handlePreviewCancel = () => {
    setShowPreview(false);
    setPreviewData([]);
    setIsProcessing(false); // 取消时重置状态
    // 可以选择清除已完成的文件
    clearCompleted();
  };

  return (
    <div className="flex flex-col h-full">
      {/* 处理中的全局遮罩层 - 包括预览加载和实际处理 */}
      {(isLoadingPreview || isProcessing) && (
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm z-50 flex items-center justify-center">
          <div className="bg-white rounded-2xl p-8 max-w-md w-full mx-4 shadow-2xl">
            <div className="text-center">
              {/* CheersAI Logo 动画 */}
              <div className="inline-flex items-center justify-center w-20 h-20 mb-4">
                <img 
                  src="/safer.png" 
                  alt="CheersAI" 
                  className={`w-full h-full object-contain ${
                    isLoadingPreview ? 'animate-spin' : 'animate-pulse'
                  }`}
                  style={isLoadingPreview ? { animationDuration: '2s' } : undefined}
                />
              </div>
              
              {isLoadingPreview ? (
                <>
                  <h3 className="text-xl font-bold text-gray-900 mb-2">正在分析文件</h3>
                  <p className="text-gray-600 mb-4">
                    正在检测敏感信息，请稍候...
                  </p>
                  <p className="text-xs text-gray-500">
                    这可能需要几秒到几分钟，取决于文件大小
                  </p>
                </>
              ) : batchStatus ? (
                <>
                  <h3 className="text-xl font-bold text-gray-900 mb-2">正在处理文件</h3>
                  <p className="text-gray-600 mb-4 truncate max-w-full">
                    {batchStatus.current_file || "准备中..."}
                  </p>
                  <div className="space-y-2">
                    <div className="flex justify-between text-sm text-gray-600">
                      <span>进度</span>
                      <span>{batchStatus.completed} / {batchStatus.total}</span>
                    </div>
                    <div className="w-full bg-gray-200 rounded-full h-2.5">
                      <div 
                        className="bg-indigo-600 h-2.5 rounded-full transition-all duration-300"
                        style={{ width: `${(batchStatus.completed / batchStatus.total) * 100}%` }}
                      ></div>
                    </div>
                    {batchStatus.failed > 0 && (
                      <p className="text-sm text-orange-600 mt-2">
                        失败: {batchStatus.failed} 个
                      </p>
                    )}
                  </div>
                  <p className="text-xs text-gray-500 mt-4">
                    ⚠️ 请勿关闭窗口或切换页面
                  </p>
                </>
              ) : (
                <>
                  <h3 className="text-xl font-bold text-gray-900 mb-2">准备处理</h3>
                  <p className="text-gray-600 mb-4">
                    正在启动脱敏任务...
                  </p>
                  <p className="text-xs text-gray-500">
                    请稍候，任务即将开始
                  </p>
                </>
              )}
              
              {/* 品牌标语 */}
              <div className="mt-6 pt-4 border-t border-gray-100">
                <p className="text-sm font-medium text-indigo-600">
                  CheersAI
                </p>
                <p className="text-xs text-gray-500 mt-1 relative overflow-hidden">
                  <span className="relative inline-block">
                    让数据留在本地，让 AI 能力走在前沿
                    {/* 光影扫过效果 */}
                    <span 
                      className="absolute inset-0 bg-gradient-to-r from-transparent via-white to-transparent opacity-40"
                      style={{
                        animation: 'shimmer 3s infinite',
                        backgroundSize: '200% 100%',
                      }}
                    />
                  </span>
                </p>
              </div>
              
              {/* 添加光影动画的 CSS */}
              <style>{`
                @keyframes shimmer {
                  0% {
                    transform: translateX(-100%);
                  }
                  100% {
                    transform: translateX(100%);
                  }
                }
              `}</style>
            </div>
          </div>
        </div>
      )}
      
      <PageHeader
        title="文件处理"
        description="拖放文件进行数据脱敏"
        actions={
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={clearCompleted}>
              <Trash2 className="w-4 h-4 mr-1" />
              清除已完成
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={handleSelectOutputDir}
            >
              <FolderOpen className="w-4 h-4 mr-1" />
              选择输出目录
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={async () => {
                try {
                  console.log("Manual database initialization...");
                  await tauriCommands.initializeDatabase();
                  console.log("Database initialized");
                  
                  console.log("Adding test log...");
                  await addLog("info", "手动测试日志", "这是一条手动添加的测试日志，用于验证数据库存储", undefined, "manual_test");
                  console.log("Test log added");
                  
                  console.log("Getting database info...");
                  const dbInfo = await tauriCommands.getDatabaseInfo();
                  console.log("Database info:", dbInfo);
                  
                  alert(`数据库测试完成！\n数据库路径: ${dbInfo.database_path}\n数据库存在: ${dbInfo.database_exists}\n日志数量: ${dbInfo.log_count}`);
                } catch (error) {
                  console.error("Database test failed:", error);
                  alert(`数据库测试失败: ${error}`);
                }
              }}
            >
              数据库测试
            </Button>
            <Button
              size="sm"
              onClick={handleStart}
              disabled={pendingCount === 0 || !outputDir || !!activeJobId || selectedRules.length === 0 || isLoadingPreview}
              className="bg-indigo-500 hover:bg-indigo-600"
            >
              {isLoadingPreview ? (
                <>
                  <svg className="animate-spin -ml-1 mr-2 h-4 w-4 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4"></circle>
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                  </svg>
                  正在分析...
                </>
              ) : (
                <>
                  <Play className="w-4 h-4 mr-1" />
                  {activeJobId ? "处理中..." : `开始处理 ${pendingCount > 0 ? `(${pendingCount})` : ""}`}
                </>
              )}
            </Button>
          </div>
        }
      />
      <div className="flex-1 overflow-auto p-6">
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          <div className="lg:col-span-2 space-y-6">
            <DropZone onFilesDropped={handleDrop} />

            <div className="space-y-4">
              <PassphraseBox
                value={passphrase}
                onChange={setPassphrase}
                label="映射加密口令（可选）"
              />
              
              {outputDir && (
                <div className="p-3 bg-green-50 border border-green-200 rounded-lg">
                  <p className="text-sm text-green-700">
                    <strong>输出目录:</strong> {getDisplayPath(outputDir, 60)}
                  </p>
                </div>
              )}

              {batchStatus && (
                <BatchProgress
                  total={batchStatus.total}
                  completed={batchStatus.completed}
                  failed={batchStatus.failed}
                  currentFile={batchStatus.current_file}
                  status={batchStatus.status}
                />
              )}
            </div>

            {files.length > 0 && (
              <div className="space-y-2">
                <p className="text-sm font-medium text-gray-700">
                  文件队列 ({files.length})
                </p>
                {files.map((f) => (
                  <FileQueueItem key={f.id} file={f} onRemove={removeFile} />
                ))}
              </div>
            )}
          </div>

          <div className="space-y-6">
            <RuleSelector
              selectedRules={selectedRules}
              onRulesChange={setSelectedRules}
            />
            
            {/* AI 检测开关 */}
            <div className="p-4 bg-gradient-to-br from-purple-50 to-indigo-50 border border-purple-200 rounded-xl">
              <div className="flex items-start justify-between">
                <div className="flex-1">
                  <h3 className="text-sm font-semibold text-purple-900 mb-1">
                    🤖 AI 多方法检测
                  </h3>
                  {isCheckingAi ? (
                    <p className="text-xs text-purple-600 mb-2">
                      正在检查 AI 配置...
                    </p>
                  ) : !aiDetectionAvailable ? (
                    <p className="text-xs text-orange-600 mb-2">
                      ⚠️ 未配置 Ollama 或模型，请先安装
                    </p>
                  ) : (
                    <p className="text-xs text-purple-700 mb-2">
                      使用 AI+NER+正则+搜索 四种方法检测敏感信息
                    </p>
                  )}
                  <p className="text-xs text-purple-600 mb-1">
                    • 姓名：四种方法<strong>交集</strong>（全部确认才脱敏）
                  </p>
                  <p className="text-xs text-purple-600">
                    • 其他：四种方法<strong>并集</strong>（任一识别即脱敏）
                  </p>
                </div>
                <label className="relative inline-flex items-center cursor-pointer ml-3">
                  <input
                    type="checkbox"
                    checked={useAiValidation}
                    onChange={(e) => setUseAiValidation(e.target.checked)}
                    disabled={!aiDetectionAvailable || isCheckingAi}
                    className="sr-only peer"
                  />
                  <div className={`w-11 h-6 ${
                    !aiDetectionAvailable || isCheckingAi 
                      ? 'bg-gray-300 cursor-not-allowed' 
                      : 'bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-purple-300'
                  } rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all ${
                    aiDetectionAvailable && !isCheckingAi ? 'peer-checked:bg-purple-600' : ''
                  }`}></div>
                </label>
              </div>
            </div>
          </div>
        </div>

        {/* 使用说明 */}
        <div className="mt-8 p-6 bg-blue-50/60 border border-blue-100 rounded-xl">
          <h3 className="text-sm font-bold text-blue-900 mb-3">使用说明</h3>
          <ol className="space-y-2 text-sm text-blue-700">
            <li>1. 拖放或点击选择需要脱敏的文件（支持 CSV、Excel、JSON、TXT、Word、PPT、PDF、Markdown）</li>
            <li>2. 在右侧选择需要启用的脱敏规则（如身份证号、手机号、邮箱等）</li>
            <li>3. 输入映射加密口令（可选，用于生成可逆的脱敏映射文件）</li>
            <li>4. 点击"选择输出目录"指定脱敏后文件的保存位置</li>
            <li>5. 点击"开始处理"，预览脱敏效果并确认后执行脱敏</li>
            <li>6. 脱敏完成后，输出目录中将生成脱敏文件和对照映射文件（.cmap）</li>
          </ol>
        </div>
      </div>

      {/* 脱敏预览对话框 */}
      <MaskingPreviewDialog
        open={showPreview}
        onOpenChange={setShowPreview}
        previews={previewData}
        onConfirm={handlePreviewConfirm}
        onCancel={handlePreviewCancel}
      />

      {/* OCR 下载对话框 */}
      <OcrDownloadDialog
        open={showOcrDownload}
        onOpenChange={setShowOcrDownload}
        onComplete={() => {
          // OCR 安装完成后，可以重试之前失败的操作
          console.log('OCR installed successfully');
        }}
      />
    </div>
  );
}
