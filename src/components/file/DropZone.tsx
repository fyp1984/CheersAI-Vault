import { useEffect, useState } from "react";
import { UploadCloud } from "lucide-react";
import { cn } from "@/lib/utils";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";

interface DropZoneProps {
  onFilesDropped: (paths: string[]) => void;
}

export function DropZone({ onFilesDropped }: DropZoneProps) {
  const [isDragActive, setIsDragActive] = useState(false);

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

  // 使用全局 listen API 监听文件拖放事件
  useEffect(() => {
    console.log("DropZone: Setting up global Tauri file drop listeners...");
    
    let unlistenEnter: (() => void) | null = null;
    let unlistenOver: (() => void) | null = null;
    let unlistenDrop: (() => void) | null = null;
    let unlistenLeave: (() => void) | null = null;
    
    // 监听文件进入窗口
    listen<string[]>('tauri://drag-enter', (event) => {
      console.log("=== DRAG ENTER ===", event);
      setIsDragActive(true);
    }).then(fn => { 
      unlistenEnter = fn;
      console.log("Registered: tauri://drag-enter");
    }).catch(err => {
      console.error("Failed to register tauri://drag-enter:", err);
    });
    
    // 监听文件在窗口上方移动
    listen<string[]>('tauri://drag-over', (event) => {
      console.log("=== DRAG OVER ===", event);
      setIsDragActive(true);
    }).then(fn => { 
      unlistenOver = fn;
      console.log("Registered: tauri://drag-over");
    }).catch(err => {
      console.error("Failed to register tauri://drag-over:", err);
    });
    
    // 监听文件拖放
    listen<string[]>('tauri://drag-drop', (event) => {
      console.log("=== DRAG DROP ===", event);
      setIsDragActive(false);
      if (event.payload && event.payload.length > 0) {
        console.log("Files dropped:", event.payload);
        onFilesDropped(event.payload);
      }
    }).then(fn => { 
      unlistenDrop = fn;
      console.log("Registered: tauri://drag-drop");
    }).catch(err => {
      console.error("Failed to register tauri://drag-drop:", err);
    });
    
    // 监听拖放离开
    listen('tauri://drag-leave', (event) => {
      console.log("=== DRAG LEAVE ===", event);
      setIsDragActive(false);
    }).then(fn => { 
      unlistenLeave = fn;
      console.log("Registered: tauri://drag-leave");
    }).catch(err => {
      console.error("Failed to register tauri://drag-leave:", err);
    });
    
    console.log("DropZone: All file drop listeners registered");
    
    return () => {
      console.log("DropZone: Cleaning up file drop listeners");
      if (unlistenEnter) unlistenEnter();
      if (unlistenOver) unlistenOver();
      if (unlistenDrop) unlistenDrop();
      if (unlistenLeave) unlistenLeave();
    };
  }, [onFilesDropped]);

  return (
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
      <p className="text-sm font-medium text-gray-600">
        点击选择文件
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
