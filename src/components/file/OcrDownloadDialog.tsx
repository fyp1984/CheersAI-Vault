import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import { AlertCircle, Download, CheckCircle2 } from 'lucide-react';

interface OcrDownloadProgress {
  downloaded: number;
  total: number;
  percentage: number;
  status: string;
}

interface OcrDownloadDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onComplete?: () => void;
}

export function OcrDownloadDialog({ open, onOpenChange, onComplete }: OcrDownloadDialogProps) {
  const [isDownloading, setIsDownloading] = useState(false);
  const [progress, setProgress] = useState<OcrDownloadProgress>({
    downloaded: 0,
    total: 0,
    percentage: 0,
    status: '准备下载...',
  });
  const [error, setError] = useState<string | null>(null);
  const [isComplete, setIsComplete] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await listen<OcrDownloadProgress>('ocr-download-progress', (event) => {
        setProgress(event.payload);
        if (event.payload.percentage >= 100 && event.payload.status.includes('完成')) {
          setIsComplete(true);
          setIsDownloading(false);
        }
      });
    };

    if (open) {
      setupListener();
    }

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [open]);

  const handleDownload = async () => {
    setIsDownloading(true);
    setError(null);
    setIsComplete(false);

    try {
      await invoke('download_ocr_package');
      // 下载完成后会通过事件通知
    } catch (err) {
      console.error('OCR download failed:', err);
      setError(err as string);
      setIsDownloading(false);
    }
  };

  const handleClose = () => {
    if (!isDownloading) {
      onOpenChange(false);
      if (isComplete && onComplete) {
        onComplete();
      }
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  };

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-[500px]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {isComplete ? (
              <>
                <CheckCircle2 className="h-5 w-5 text-green-500" />
                OCR 安装完成
              </>
            ) : (
              <>
                <Download className="h-5 w-5" />
                下载 OCR 依赖
              </>
            )}
          </DialogTitle>
          <DialogDescription>
            {isComplete
              ? 'OCR 功能已成功安装，现在可以处理扫描版 PDF 了。'
              : '检测到扫描版 PDF，需要下载 Python 运行时和 OCR 依赖才能识别文字。'}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {!isDownloading && !isComplete && !error && (
            <div className="space-y-3">
              <div className="flex items-start gap-2 text-sm text-muted-foreground">
                <AlertCircle className="h-4 w-4 mt-0.5 flex-shrink-0" />
                <div>
                  <p className="font-medium mb-1">下载说明：</p>
                  <ul className="list-disc list-inside space-y-1 text-xs">
                    <li>将从 Python 官方源下载运行时和 OCR 依赖</li>
                    <li>总大小约 100MB，首次下载需要 3-5 分钟</li>
                    <li>下载后自动安装，无需重启应用</li>
                    <li>仅需下载一次，后续可直接使用</li>
                  </ul>
                </div>
              </div>
            </div>
          )}

          {isDownloading && (
            <div className="space-y-3">
              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <span className="text-muted-foreground">{progress.status}</span>
                  <span className="font-medium">{progress.percentage.toFixed(1)}%</span>
                </div>
                <Progress value={progress.percentage} className="h-2" />
                {progress.total > 0 && (
                  <div className="text-xs text-muted-foreground text-center">
                    {formatBytes(progress.downloaded)} / {formatBytes(progress.total)}
                  </div>
                )}
              </div>
            </div>
          )}

          {isComplete && (
            <div className="flex items-center gap-2 p-3 bg-green-50 border border-green-200 rounded-md">
              <CheckCircle2 className="h-5 w-5 text-green-500 flex-shrink-0" />
              <div className="text-sm text-green-700">
                OCR 功能已就绪，可以开始处理扫描版 PDF 文件了。
              </div>
            </div>
          )}

          {error && (
            <div className="flex items-start gap-2 p-3 bg-red-50 border border-red-200 rounded-md">
              <AlertCircle className="h-5 w-5 text-red-500 flex-shrink-0 mt-0.5" />
              <div className="text-sm text-red-700">
                <p className="font-medium mb-1">下载失败</p>
                <p className="text-xs">{error}</p>
              </div>
            </div>
          )}
        </div>

        <DialogFooter>
          {!isDownloading && !isComplete && (
            <>
              <Button variant="outline" onClick={handleClose}>
                取消
              </Button>
              <Button onClick={handleDownload}>
                <Download className="h-4 w-4 mr-2" />
                开始下载
              </Button>
            </>
          )}

          {isDownloading && (
            <Button disabled>
              <div className="flex items-center gap-2">
                <div className="h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                下载中...
              </div>
            </Button>
          )}

          {isComplete && (
            <Button onClick={handleClose}>
              <CheckCircle2 className="h-4 w-4 mr-2" />
              完成
            </Button>
          )}

          {error && (
            <>
              <Button variant="outline" onClick={handleClose}>
                取消
              </Button>
              <Button onClick={handleDownload}>
                重试
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
