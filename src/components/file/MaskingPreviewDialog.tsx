import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Eye, Save, RotateCcw, Search } from "lucide-react";
import { useState, useEffect } from "react";
import type { PreviewResult } from "@/types/commands";
import { FindReplaceDialog } from "./FindReplaceDialog";

interface FilePreview {
  fileName: string;
  preview: PreviewResult;
}

// 简单的文本差异高亮组件
function DiffLine({ original, masked }: { original: string; masked: string }) {
  // 使用简单的字符串分割来找出差异
  const parts: Array<{ text: string; type: 'same' | 'removed' | 'added' }> = [];
  
  // 简单实现：按空格分词，找出不同的部分
  const originalWords = original.split(/(\s+)/);
  const maskedWords = masked.split(/(\s+)/);
  
  // 如果长度相同，逐个对比
  if (originalWords.length === maskedWords.length) {
    for (let i = 0; i < originalWords.length; i++) {
      if (originalWords[i] === maskedWords[i]) {
        parts.push({ text: maskedWords[i], type: 'same' });
      } else {
        parts.push({ text: originalWords[i], type: 'removed' });
        parts.push({ text: maskedWords[i], type: 'added' });
      }
    }
  } else {
    // 长度不同，直接显示原文和脱敏后的文本
    return (
      <div className="space-y-1">
        <div className="text-gray-400 line-through text-xs">
          {original}
        </div>
        <div className="text-blue-600 font-medium">
          {masked}
        </div>
      </div>
    );
  }
  
  return (
    <div className="leading-relaxed">
      {parts.map((part, idx) => {
        if (part.type === 'same') {
          return <span key={idx} className="text-gray-700">{part.text}</span>;
        } else if (part.type === 'removed') {
          return (
            <span key={idx} className="bg-red-100 text-red-700 line-through px-1 rounded">
              {part.text}
            </span>
          );
        } else {
          return (
            <span key={idx} className="bg-green-100 text-green-700 font-medium px-1 rounded">
              {part.text}
            </span>
          );
        }
      })}
    </div>
  );
}

export interface ManualReplacement {
  find: string;
  replace: string;
}

interface MaskingPreviewDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  previews: FilePreview[];
  onConfirm: (manualReplacements: ManualReplacement[]) => void;
  onCancel: () => void;
}

export function MaskingPreviewDialog({
  open,
  onOpenChange,
  previews,
  onConfirm,
  onCancel,
}: MaskingPreviewDialogProps) {
  const [currentFileIndex, setCurrentFileIndex] = useState(0);
  const [findReplaceOpen, setFindReplaceOpen] = useState(false);
  const [modifiedPreviews, setModifiedPreviews] = useState(previews);
  const [manualReplacements, setManualReplacements] = useState<ManualReplacement[]>([]);

  // 同步 previews 状态
  useEffect(() => {
    setModifiedPreviews(previews);
    setManualReplacements([]);
    setCurrentFileIndex(0);
  }, [previews]);

  if (!previews || previews.length === 0) {
    console.log('MaskingPreviewDialog: No previews');
    return null;
  }
  if (!modifiedPreviews || modifiedPreviews.length === 0) {
    console.log('MaskingPreviewDialog: No modified previews');
    return null;
  }
  if (currentFileIndex >= modifiedPreviews.length) {
    console.log('MaskingPreviewDialog: Index out of bounds', currentFileIndex, modifiedPreviews.length);
    return null;
  }

  const currentFile = modifiedPreviews[currentFileIndex];
  if (!currentFile || !currentFile.preview) {
    console.log('MaskingPreviewDialog: Invalid current file', currentFile);
    return null;
  }
  
  const preview = currentFile.preview;
  const fileName = currentFile.fileName;
  
  console.log('MaskingPreviewDialog: Rendering preview', { fileName, preview });

  // 计算脱敏数量
  const maskedCount = preview.masked_rows.reduce((count, row, rowIndex) => {
    return count + row.filter((cell, cellIndex) => 
      cell !== preview.original_rows[rowIndex]?.[cellIndex]
    ).length;
  }, 0);

  // 处理查找替换：
  // 1. 优先在当前 masked_rows 中搜索（包含历次手动替换的累积结果，避免二次替换回退）
  // 2. 当前 masked 中未找到时，fallback 到 original_rows（处理已被自动脱敏为占位符的情况）
  const handleReplace = (findText: string, replaceText: string): number => {
    if (!findText.trim()) return 0;
    let count = 0;
    const newPreviews = modifiedPreviews.map((fp, idx) => {
      if (idx !== currentFileIndex) return fp;
      const newMasked = fp.preview.masked_rows.map((row, rowIdx) => {
        const origRow = fp.preview.original_rows[rowIdx] ?? [];
        return row.map((cell, cellIdx) => {
          const origCell = origRow[cellIdx] ?? '';
          // 先在当前 masked 内容（含前次手动替换结果）里找
          if (cell.includes(findText)) {
            count += cell.split(findText).length - 1;
            return cell.split(findText).join(replaceText);
          }
          // 找不到时 fallback：原文有但被自动脱敏掉了，以原文为基础替换
          if (origCell.includes(findText)) {
            count += origCell.split(findText).length - 1;
            return origCell.split(findText).join(replaceText);
          }
          return cell;
        });
      });
      return { ...fp, preview: { ...fp.preview, masked_rows: newMasked } };
    });
    setManualReplacements(prev => [
      ...prev.filter(r => r.find !== findText),
      { find: findText, replace: replaceText },
    ]);
    setModifiedPreviews(newPreviews);
    return count;
  };

  // 收集所有检测到的实体
  const allEntities = preview.detected_entities?.flatMap(row => row.entities) || [];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-4xl max-h-[80vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Eye className="w-5 h-5" />
            文件脱敏预览
          </DialogTitle>
        </DialogHeader>

        {/* 文件选项卡 */}
        {previews.length > 1 && (
          <div className="flex gap-2 overflow-x-auto pb-2 border-b">
            {previews.map((_, index) => (
              <button
                key={index}
                onClick={() => setCurrentFileIndex(index)}
                className={`px-4 py-2 rounded-lg text-sm font-medium whitespace-nowrap transition-all ${
                  index === currentFileIndex
                    ? 'bg-blue-600 text-white'
                    : 'bg-gray-100 text-gray-700 hover:bg-gray-200'
                }`}
              >
                文件{index + 1}
              </button>
            ))}
          </div>
        )}

        <div className="flex-1 overflow-hidden flex flex-col gap-4">
          {/* 数据概览 */}
          <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
            <div className="flex items-center justify-between mb-2">
              <div className="flex items-center gap-2">
                <Eye className="w-4 h-4 text-blue-600" />
                <span className="text-sm font-medium text-blue-900">数据概览</span>
              </div>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setFindReplaceOpen(true)}
                className="flex items-center gap-2 text-xs"
              >
                <Search className="w-3 h-3" />
                查找替换
              </Button>
            </div>
            <div className="text-xs text-blue-700 space-y-1">
              <p>文件名称: {fileName}</p>
              <p>预览行数: {preview.masked_rows.length} 行</p>
              <p>脱敏处理: {maskedCount} 处</p>
            </div>
          </div>

          {/* 脱敏内容预览 */}
          <ScrollArea className="h-[400px] border rounded-lg bg-white">
            <div className="p-6 font-mono text-sm leading-relaxed">
              {preview.masked_rows.map((row, rowIndex) => {
                // 将多列数据用逗号连接成一行（适配 CSV 格式）
                const originalLine = preview.original_rows[rowIndex]?.join(", ") || "";
                const maskedLine = row.join(", ") || "";
                
                // 如果行内容相同，直接显示
                if (originalLine === maskedLine) {
                  return (
                    <div key={rowIndex} className="mb-1">
                      <div className="text-gray-700">{maskedLine}</div>
                    </div>
                  );
                }
                
                // 如果行内容不同，需要高亮显示差异
                return (
                  <div key={rowIndex} className="mb-2">
                    <div className="flex items-start gap-2">
                      <span className="text-xs bg-yellow-100 text-yellow-800 px-2 py-0.5 rounded shrink-0">
                        已脱敏
                      </span>
                      <div className="flex-1">
                        <DiffLine original={originalLine} masked={maskedLine} />
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </ScrollArea>
        </div>

        <DialogFooter className="flex gap-2">
          <Button
            variant="outline"
            onClick={onCancel}
            className="flex items-center gap-2"
          >
            <RotateCcw className="w-4 h-4" />
            重新开始
          </Button>
          <Button
            onClick={() => onConfirm(manualReplacements)}
            className="bg-blue-600 hover:bg-blue-700 flex items-center gap-2"
          >
            <Save className="w-4 h-4" />
            {manualReplacements.length > 0
              ? `执行脱敏并保存（含 ${manualReplacements.length} 条手动替换）`
              : "执行脱敏并保存"}
          </Button>
        </DialogFooter>
      </DialogContent>

      {/* 查找替换对话框 */}
      <FindReplaceDialog
        open={findReplaceOpen}
        onOpenChange={setFindReplaceOpen}
        onReplace={handleReplace}
        detectedEntities={allEntities}
      />
    </Dialog>
  );
}
