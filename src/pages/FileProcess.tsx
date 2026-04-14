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

        // 如果任务完成，停止轮询
        if (status.status === "Completed" || status.status === "Failed" || status.status === "Cancelled") {
          setActiveJob(null);
          setTimeout(() => setBatchStatus(null), 3000);
        }
      } catch (error) {
        console.error("Failed to get batch status:", error);
        setActiveJob(null);
        setBatchStatus(null);
      }
    }, 1000);

    return () => clearInterval(interval);
  }, [activeJobId, files, updateFile, setActiveJob]);

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
          });
          return {
            fileName: file.name,
            preview,
          };
        })
      );
      setPreviewData(previews);
      setShowPreview(true);
    } catch (error) {
      console.error("Failed to load preview:", error);
      
      // 检查是否是 OCR 错误
      if (isOcrError(error)) {
        setShowOcrDownload(true);
      } else {
        alert(`加载预览失败: ${error}`);
      }
    } finally {
      setIsLoadingPreview(false);
    }
  };

  const executeActualMasking = async (manualReplacements: ManualReplacement[] = []) => {
    // 检查是否选择了规则
    if (selectedRules.length === 0 && manualReplacements.length === 0) {
      alert("请至少选择一个脱敏规则或添加手动替换规则");
      return;
    }

    await addLog("info", `开始批处理 ${pendingCount} 个文件`, `输出目录: ${outputDir}`, undefined, "batch_start");

    try {
      const customRulesForBatch = rules
        .filter((r) => !r.builtin && r.enabled && selectedRules.includes(r.id))
        .map((r) => ({
          id: r.id,
          name: r.name,
          pattern: r.pattern,
          replacement_template: r.replacement_template,
          use_counter: r.use_counter,
        }));

      // 将手动查找替换条目转成精确匹配自定义规则（固定文本、不追加序号）
      const manualRules = manualReplacements
        .filter(mr => mr.find.trim())
        .map((mr, i) => ({
          id: `manual_replace_${i}`,
          name: `手动替换: ${mr.find}`,
          pattern: mr.find.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), // 转义为字面量
          replacement_template: mr.replace,
          use_counter: false,
        }));

      const allCustomRules = [...customRulesForBatch, ...manualRules];
      const allRuleIds = [
        ...selectedRules,
        ...manualRules.map(r => r.id),
      ];

      const jobId = await tauriCommands.startBatchJob({
        file_paths: files.filter(f => f.status === "pending").map(f => f.path),
        output_dir: outputDir,
        rule_ids: allRuleIds,
        passphrase: passphrase || undefined,
        custom_rules: allCustomRules.length > 0 ? allCustomRules : undefined,
      });

      setActiveJob(jobId);
      
      // 将所有待处理文件状态设为处理中
      files.forEach(file => {
        if (file.status === "pending") {
          updateFile(file.id, { status: "processing" });
        }
      });
    } catch (error) {
      console.error("Failed to start batch job:", error);
      await addLog("error", "启动批处理失败", `错误: ${error}`, undefined, "batch_error");
      
      // 检查是否是 OCR 错误
      if (isOcrError(error)) {
        setShowOcrDownload(true);
      } else {
        alert(`启动批处理失败: ${error}`);
      }
    }
  };

  const pendingCount = files.filter((f) => f.status === "pending").length;

  const handlePreviewConfirm = async (manualReplacements: ManualReplacement[]) => {
    setShowPreview(false);
    setPreviewData([]);
    await executeActualMasking(manualReplacements);
  };

  const handlePreviewCancel = () => {
    setShowPreview(false);
    setPreviewData([]);
    // 可以选择清除已完成的文件
    clearCompleted();
  };

  return (
    <div className="flex flex-col h-full">
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
