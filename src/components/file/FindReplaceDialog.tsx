import { useState, useEffect, useRef } from 'react';
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Search, Plus, Trash2, X, CheckCheck, Lightbulb, AlertTriangle, Check } from 'lucide-react';
import { Badge } from '@/components/ui/badge';

interface ReplaceEntry { id: number; find: string; replace: string; }

interface FindReplaceDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onReplace: (findText: string, replaceText: string) => number;
  detectedEntities?: Array<{ text: string; entity_type: string }>;
  onRemoveEntity?: (text: string) => void;
}

const TYPE_DEFAULTS: Record<string, string> = {
  '姓名': '姓名',
  '手机号': '***PHONE***',
  '邮箱': '***EMAIL***',
  '身份证号': '***IDCARD***',
  '银行卡号': '***BANKCARD***',
  'IP地址': '***IP***',
  '护照号': '***PASSPORT***',
  '地址': '***ADDRESS***',
  '日期': '***DATE***',
  '金额': '***AMOUNT***',
};

let _uid = 1;
const uid = () => _uid++;

export function FindReplaceDialog({
  open,
  onOpenChange,
  onReplace,
  detectedEntities = [],
  onRemoveEntity,
}: FindReplaceDialogProps) {
  const [entries, setEntries] = useState<ReplaceEntry[]>([
    { id: uid(), find: '', replace: '' },
  ]);
  const [feedback, setFeedback] = useState<{ total: number; notFound: string[] } | null>(null);
  const [localEntities, setLocalEntities] = useState(detectedEntities);
  
  // 用于滚动到最新添加的条目
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const lastEntryRef = useRef<HTMLDivElement>(null);

  // 当 detectedEntities 变化时更新本地状态
  useEffect(() => {
    setLocalEntities(detectedEntities);
  }, [detectedEntities]);

  // 当条目数量变化时，滚动到最新添加的条目
  useEffect(() => {
    if (lastEntryRef.current && scrollContainerRef.current) {
      lastEntryRef.current.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
  }, [entries.length]);

  const uniqueEntities = Array.from(
    new Map(localEntities.map(e => [e.text, e])).values()
  );

  const handleRemoveEntity = (text: string, e: React.MouseEvent) => {
    e.stopPropagation(); // 阻止冒泡，避免触发点击添加
    setLocalEntities(prev => prev.filter(entity => entity.text !== text));
    if (onRemoveEntity) {
      onRemoveEntity(text);
    }
  };

  const handleQuickAdd = (text: string, type: string, e: React.MouseEvent) => {
    e.stopPropagation(); // 阻止冒泡
    handleQuickSelect(text, type);
    // 添加后从识别列表中移除
    setLocalEntities(prev => prev.filter(entity => entity.text !== text));
  };

  const updateEntry = (id: number, field: 'find' | 'replace', value: string) => {
    setFeedback(null);
    setEntries(prev => prev.map(e => e.id === id ? { ...e, [field]: value } : e));
  };

  const addEntry = () =>
    setEntries(prev => [...prev, { id: uid(), find: '', replace: '' }]);

  const removeEntry = (id: number) =>
    setEntries(prev => prev.length > 1 ? prev.filter(e => e.id !== id) : prev);

  const handleQuickSelect = (text: string, type: string) => {
    setFeedback(null);
    const last = entries[entries.length - 1];
    if (!last.find) {
      setEntries(prev => prev.map(e => e.id === last.id
        ? { ...e, find: text, replace: TYPE_DEFAULTS[type] || '***MASKED***' }
        : e
      ));
    } else {
      setEntries(prev => [...prev, { id: uid(), find: text, replace: TYPE_DEFAULTS[type] || '***MASKED***' }]);
    }
    // 添加后从识别列表中移除
    setLocalEntities(prev => prev.filter(entity => entity.text !== text));
  };

  const handleApplyAll = () => {
    const valid = entries.filter(e => e.find.trim());
    if (valid.length === 0) return;

    let total = 0;
    const notFound: string[] = [];
    valid.forEach(e => {
      const n = onReplace(e.find, e.replace);
      total += n;
      if (n === 0) notFound.push(e.find);
    });

    if (total === 0) {
      setFeedback({ total: 0, notFound });
      return; // 不关闭，让用户看到提示
    }

    setFeedback(null);
    onOpenChange(false);
    setEntries([{ id: uid(), find: '', replace: '' }]);
  };

  const validCount = entries.filter(e => e.find.trim()).length;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[85vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Search className="w-5 h-5" />
            手动查找替换
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-hidden flex flex-col gap-4">
          {/* NER 快捷标签 */}
          {uniqueEntities.length > 0 ? (
            <div className="space-y-1.5">
              <Label className="text-xs">识别到的敏感词（点击添加替换条目）</Label>
              <div className="flex flex-wrap gap-1.5 p-2.5 bg-blue-50 border border-blue-200 rounded-lg max-h-28 overflow-y-auto">
                {uniqueEntities.map((entity, idx) => (
                  <Badge
                    key={idx}
                    variant="outline"
                    className="cursor-pointer hover:bg-blue-100 border-blue-200 transition-colors text-xs relative group pr-6 pl-6"
                    onClick={() => handleQuickSelect(entity.text, entity.entity_type)}
                  >
                    <button
                      onClick={(e) => handleQuickAdd(entity.text, entity.entity_type, e)}
                      className="absolute -top-1 -left-1 w-4 h-4 bg-green-500 hover:bg-green-600 text-white rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
                      title="添加到替换列表"
                    >
                      <Check className="w-3 h-3" />
                    </button>
                    <span className="text-orange-700 font-medium mr-1">{entity.entity_type}:</span>
                    <span className="text-gray-700">{entity.text}</span>
                    <button
                      onClick={(e) => handleRemoveEntity(entity.text, e)}
                      className="absolute -top-1 -right-1 w-4 h-4 bg-red-500 hover:bg-red-600 text-white rounded-full flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
                      title="删除此识别结果"
                    >
                      <X className="w-3 h-3" />
                    </button>
                  </Badge>
                ))}
              </div>
            </div>
          ) : (
            <div className="p-3 bg-gray-50 border border-gray-200 rounded-lg text-xs text-gray-500 flex items-start gap-2">
              <Lightbulb className="w-4 h-4 text-gray-400 flex-shrink-0 mt-0.5" />
              <span>未检测到标准 PII（手机号/身份证等）。可在下方手动输入要替换的文本，
              或前往<strong>规则配置</strong>添加自定义正则规则后重新处理。</span>
            </div>
          )}

          {/* 替换条目列表 */}
          <div className="space-y-1.5 flex-1 overflow-hidden flex flex-col">
            <div className="flex items-center justify-between flex-shrink-0">
              <Label className="text-xs">替换列表</Label>
              <Button size="sm" variant="ghost" onClick={addEntry} className="text-xs h-6 px-2 gap-1">
                <Plus className="w-3 h-3" /> 添加条目
              </Button>
            </div>
            <div ref={scrollContainerRef} className="flex-1 overflow-y-auto pr-2">
              <div className="space-y-2">
                {entries.map((entry, idx) => (
                  <div 
                    key={entry.id} 
                    ref={idx === entries.length - 1 ? lastEntryRef : null}
                    className="flex items-center gap-2"
                  >
                    <span className="text-xs text-gray-400 w-4 text-right shrink-0">{idx + 1}</span>
                    <Input
                      placeholder="查找内容…"
                      value={entry.find}
                      onChange={(e) => updateEntry(entry.id, 'find', e.target.value)}
                      className="text-sm h-8 font-mono flex-1"
                    />
                    <span className="text-gray-400 text-xs shrink-0">→</span>
                    <Input
                      placeholder="替换为…"
                      value={entry.replace}
                      onChange={(e) => updateEntry(entry.id, 'replace', e.target.value)}
                      className="text-sm h-8 font-mono flex-1"
                    />
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => removeEntry(entry.id)}
                      className="p-1 h-7 w-7 text-gray-400 hover:text-red-500 shrink-0"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          </div>

          {/* 预览摘要 */}
          {feedback && feedback.total === 0 && (
            <div className="bg-red-50 border border-red-300 rounded-lg p-2.5 text-xs text-red-700 space-y-1">
              <p className="font-medium flex items-center gap-1.5">
                <AlertTriangle className="w-4 h-4" />
                未找到匹配内容，替换未执行
              </p>
              <p>以下文本在当前预览中不存在（可能已被脱敏规则替换为占位符）：</p>
              <ul className="list-disc list-inside">
                {feedback.notFound.map((t, i) => <li key={i} className="font-mono">{t}</li>)}
              </ul>
              <p className="text-gray-500">提示：查找替换作用于已脱敏后的内容。若文字已被规则处理，请直接搜索占位符（如 姓名1）。</p>
            </div>
          )}
          {validCount > 0 && !feedback && (
            <div className="bg-blue-50 border border-blue-200 rounded-lg p-2.5 text-xs text-blue-800">
              将对预览内容执行 <strong>{validCount}</strong> 条替换操作（一次性全部替换）
            </div>
          )}
        </div>

        <DialogFooter className="flex gap-2 pt-2">
          <Button variant="outline" onClick={() => onOpenChange(false)} className="flex items-center gap-1.5 text-sm">
            <X className="w-4 h-4" /> 取消
          </Button>
          <Button
            onClick={handleApplyAll}
            disabled={validCount === 0}
            className="bg-blue-600 hover:bg-blue-700 flex items-center gap-1.5 text-sm"
          >
            <CheckCheck className="w-4 h-4" />
            全部替换（{validCount} 条）
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
