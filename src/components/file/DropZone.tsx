import { useCallback, useEffect, useRef, useState } from "react";
import { UploadCloud } from "lucide-react";
import { cn } from "@/lib/utils";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface DropZoneProps {
  onFilesDropped: (paths: string[]) => void;
}

export function DropZone({ onFilesDropped }: DropZoneProps) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const [isDragActive, setIsDragActive] = useState(false);

  const isInsideDropZone = useCallback((position: { x: number; y: number }) => {
    const rect = rootRef.current?.getBoundingClientRect();
    if (!rect) return false;

    const scale = window.devicePixelRatio || 1;
    const x = position.x / scale;
    const y = position.y / scale;

    return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
  }, []);

  const handleFileSelect = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          {
            name: "支持的文件",
            extensions: ["csv", "xlsx", "xls", "json", "txt", "docx", "doc", "pptx", "ppt", "pdf", "md", "markdown"]
          }
        ]
      });
      
      if (selected) {
        const paths = Array.isArray(selected) ? selected : [selected];
        console.log("Selected files via dialog:", paths);
        onFilesDropped(paths);
      }
    } catch (error) {
      console.error("Failed to open file dialog:", error);
    }
  };

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const setupDragDropListener = async () => {
      try {
        console.log("Setting up Tauri drag-drop listener...");
        unlisten = await getCurrentWindow().onDragDropEvent((event) => {
          if (disposed) return;

          console.log("Tauri drag-drop event:", event.payload.type, event.payload);

          if (event.payload.type === "leave") {
            setIsDragActive(false);
            return;
          }

          if (event.payload.type === "enter" || event.payload.type === "over") {
            const inside = isInsideDropZone(event.payload.position);
            setIsDragActive(inside);
            return;
          }

          if (event.payload.type === "drop") {
            const inside = isInsideDropZone(event.payload.position);
            setIsDragActive(false);

            if (inside && event.payload.paths.length > 0) {
              console.log("Files dropped via Tauri:", event.payload.paths);
              onFilesDropped(event.payload.paths);
            }
          }
        });
        console.log("Tauri drag-drop listener set up successfully");
      } catch (error) {
        console.error("Failed to listen for Tauri drag-drop events:", error);
      }
    };

    setupDragDropListener();

    return () => {
      disposed = true;
      setIsDragActive(false);
      unlisten?.();
    };
  }, [isInsideDropZone, onFilesDropped]);

  return (
    <div
      ref={rootRef}
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
      <p className="text-sm font-medium text-gray-600">
        {isDragActive ? "释放文件以添加" : "拖放文件到此处，或点击选择"}
      </p>
      <p className="mt-1 text-xs text-gray-400">
        支持 CSV、Excel、JSON、TXT、Word、PowerPoint、PDF、Markdown
      </p>
      <p className="mt-0.5 text-xs text-gray-400">
        注：Word/PPT/PDF 将输出为 TXT 格式
      </p>
    </div>
  );
}
