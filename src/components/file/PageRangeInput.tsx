import { useState, useEffect } from "react";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { ArrowRight } from "lucide-react";

interface PageRangeInputProps {
  fileName: string;
  fileFormat: string;
  value?: [number, number];
  onChange: (range?: [number, number]) => void;
  totalPages?: number; // 文件总页数
}

// 支持分页的文件格式
const PAGEABLE_FORMATS = ['.pdf', '.docx', '.doc', '.pptx', '.ppt'];

export function PageRangeInput({ fileName, fileFormat, value, onChange, totalPages }: PageRangeInputProps) {
  const [startValue, setStartValue] = useState("");
  const [endValue, setEndValue] = useState("");
  const [error, setError] = useState("");
  const [hasUserEdited, setHasUserEdited] = useState(false);

  // 检查文件是否支持分页
  const isPageable = PAGEABLE_FORMATS.some(ext => 
    fileName.toLowerCase().endsWith(ext) || fileFormat.toLowerCase().includes(ext.replace('.', ''))
  );

  useEffect(() => {
    if (hasUserEdited) {
      return;
    }

    if (value) {
      setStartValue(String(value[0]));
      setEndValue(String(value[1]));
    } else {
      setStartValue("");
      setEndValue("");
    }
  }, [value, hasUserEdited]);

  // 如果不支持分页，不显示组件
  if (!isPageable) {
    return null;
  }

  const commitRange = (nextStart: string, nextEnd: string) => {
    const startText = nextStart.trim();
    const endText = nextEnd.trim();
    setError("");

    if (!startText && !endText) {
      // 两个都为空时使用默认范围：从第 1 页到最后一页，等价于处理全部页
      console.log('📄 Page range defaults to full document for:', fileName);
      onChange(undefined);
      return;
    }

    if (startText && !/^\d+$/.test(startText)) {
      setError("起始页需为数字");
      onChange(undefined);
      return;
    }

    if (endText && !/^\d+$/.test(endText)) {
      setError("结束页需为数字");
      onChange(undefined);
      return;
    }

    if (!endText && !totalPages) {
      setError("请输入结束页");
      onChange(undefined);
      return;
    }

    const start = startText ? parseInt(startText, 10) : 1;
    const end = endText ? parseInt(endText, 10) : totalPages!;

    if (start < 1) {
      setError("起始页必须 >= 1");
      onChange(undefined);
      return;
    }

    if (start > end) {
      setError("起始页不能大于结束页");
      onChange(undefined);
      return;
    }

    // 如果有总页数，验证是否超出范围
    if (totalPages && end > totalPages) {
      setError(`页码超出总页数 ${totalPages}`);
      onChange(undefined);
      return;
    }

    // 验证通过
    console.log('✅ Page range set for:', fileName, 'range:', [start, end]);
    onChange([start, end]);
  };

  const handleStartChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const nextStart = e.target.value;
    setHasUserEdited(true);
    setStartValue(nextStart);
    commitRange(nextStart, endValue);
  };

  const handleEndChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const nextEnd = e.target.value;
    setHasUserEdited(true);
    setEndValue(nextEnd);
    commitRange(startValue, nextEnd);
  };

  return (
    <div className="ml-auto flex shrink-0 flex-col items-end gap-1">
      <div
        className={cn(
          "flex h-9 items-center gap-1.5 rounded-lg border border-blue-200 bg-blue-50/70 px-2.5 shadow-[0_1px_0_rgba(37,99,235,0.08)] transition-colors",
          "focus-within:border-blue-400 focus-within:bg-white focus-within:ring-2 focus-within:ring-blue-100",
          error && "border-red-300 bg-red-50/70 focus-within:border-red-400 focus-within:ring-red-100"
        )}
      >
        <span className={cn("whitespace-nowrap text-xs font-semibold text-blue-700", error && "text-red-600")}>
          页码
        </span>
        <Input
          type="text"
          inputMode="numeric"
          placeholder="1"
          value={startValue}
          onChange={handleStartChange}
          className={cn(
            "h-6 w-11 rounded-md border-blue-200 bg-white px-1 text-center text-sm font-semibold text-blue-700 shadow-none",
            "placeholder:text-blue-300 focus-visible:border-blue-400 focus-visible:ring-1 focus-visible:ring-blue-100",
            error && "border-red-300 text-red-600 focus-visible:border-red-400 focus-visible:ring-red-100"
          )}
        />
        <ArrowRight className={cn("h-3.5 w-3.5 text-blue-400", error && "text-red-400")} />
        <Input
          type="text"
          inputMode="numeric"
          placeholder={totalPages ? String(totalPages) : "10"}
          value={endValue}
          onChange={handleEndChange}
          className={cn(
            "h-6 w-11 rounded-md border-blue-200 bg-white px-1 text-center text-sm font-semibold text-blue-700 shadow-none",
            "placeholder:text-blue-300 focus-visible:border-blue-400 focus-visible:ring-1 focus-visible:ring-blue-100",
            error && "border-red-300 text-red-600 focus-visible:border-red-400 focus-visible:ring-red-100"
          )}
        />
        {totalPages && (
          <span className="ml-1 whitespace-nowrap rounded-full bg-blue-100 px-2 py-0.5 text-xs font-medium text-blue-700">
            共 {totalPages} 页
          </span>
        )}
      </div>
      {error && <p className="text-xs text-red-500">{error}</p>}
    </div>
  );
}
