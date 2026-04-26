import { ScrollArea } from "@/components/ui/scroll-area";
import type { PreviewResult } from "@/types/commands";

interface PreviewPanelProps {
  preview: PreviewResult | null;
}

export function PreviewPanel({ preview }: PreviewPanelProps) {
  if (!preview) {
    return (
      <div className="flex items-center justify-center h-full text-gray-400 text-sm">
        选择文件后预览脱敏效果
      </div>
    );
  }

  return (
    <ScrollArea className="h-full">
      <div className="p-4">
        <table className="w-full text-xs border-collapse">
          <thead>
            <tr>
              {preview.headers.map((h, i) => (
                <th key={i} className="border border-gray-200 px-2 py-1 bg-gray-50 text-left font-medium text-gray-600">
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {preview.masked_rows.map((row, ri) => (
              <tr key={ri} className="hover:bg-gray-50">
                {row.map((cell, ci) => (
                  <td key={ci} className="border border-gray-200 px-2 py-1 text-gray-700">
                    {cell !== preview.original_rows[ri]?.[ci] ? (
                      <span className="text-indigo-600 font-medium">{cell}</span>
                    ) : (
                      cell
                    )}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </ScrollArea>
  );
}
