import { useEffect, useState } from 'react';
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
import { AlertCircle, CheckCircle, CheckCircle2, Download, Maximize2, Minimize2 } from 'lucide-react';

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

const initialProgress: OcrDownloadProgress = {
  downloaded: 0,
  total: 0,
  percentage: 0,
  status: '准备下载...',
};

export function OcrDownloadDialog({ open, onOpenChange, onComplete }: OcrDownloadDialogProps) {
  const [isDownloading, setIsDownloading] = useState(false);
  const [progress, setProgress] = useState<OcrDownloadProgress>(initialProgress);
  const [error, setError] = useState<string | null>(null);
  const [isComplete, setIsComplete] = useState(false);
  const [isChecking, setIsChecking] = useState(true);

  const resetProgress = () => setProgress(initialProgress);

  useEffect(() => {
    if (!open || isDownloading || isComplete || error) {
      return;
    }

    const checkInstallation = async () => {
      setIsChecking(true);
      try {
        const installed = await invoke<boolean>('check_ocr_installed');
        console.log('OCR installation check:', installed);
        if (installed) {
          console.log('OCR already installed, closing dialog');
          onOpenChange(false);
          return;
        }

        setIsComplete(false);
        setError(null);
        resetProgress();
      } catch (err) {
        console.error('Failed to check OCR installation:', err);
      } finally {
        setIsChecking(false);
      }
    };

    checkInstallation();
  }, [open, isDownloading, isComplete, error, onOpenChange]);

  useEffect(() => {
    if (!open && !isDownloading) {
      return;
    }

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

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [open, isDownloading]);

  useEffect(() => {
    if (open || !isComplete || isDownloading) {
      return;
    }

    const timer = window.setTimeout(() => {
      onComplete?.();
      setIsComplete(false);
      resetProgress();
    }, 800);

    return () => window.clearTimeout(timer);
  }, [open, isComplete, isDownloading, onComplete]);

  const handleDownload = async () => {
    setIsDownloading(true);
    setError(null);
    setIsComplete(false);
    resetProgress();

    try {
      await invoke('download_ocr_package');
      setProgress((current) => ({
        ...current,
        percentage: 100,
        status: current.status.includes('完成') ? current.status : 'OCR 安装完成',
      }));
      setIsComplete(true);
      setIsDownloading(false);
    } catch (err) {
      console.error('OCR download failed:', err);
      setError(err as string);
      setIsDownloading(false);
    }
  };

  const handleClose = () => {
    if (isDownloading) {
      onOpenChange(false);
      return;
    }

    onOpenChange(false);
    if (isComplete && onComplete) {
      onComplete();
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  };

  const progressValue = Math.max(0, Math.min(100, progress.percentage || 0));
  const hasBackgroundStatus = !open && (isDownloading || error);

  return (
    <>
      {hasBackgroundStatus && (
        <div className="fixed bottom-6 right-6 z-50 w-[360px] max-w-[calc(100vw-48px)] rounded-lg border bg-background p-4 shadow-lg">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0 space-y-1">
              <div className="flex items-center gap-2 text-sm font-medium">
                {isComplete ? (
                  <CheckCircle2 className="h-4 w-4 text-blue-500" />
                ) : error ? (
                  <AlertCircle className="h-4 w-4 text-red-500" />
                ) : (
                  <Download className="h-4 w-4 text-primary" />
                )}
                <span>{isComplete ? 'OCR 安装完成' : error ? 'OCR 下载失败' : 'OCR 后台下载中'}</span>
              </div>
              <p className="truncate text-xs text-muted-foreground">{error || progress.status}</p>
            </div>
            <Button variant="ghost" size="sm" className="h-8 px-2" onClick={() => onOpenChange(true)}>
              <Maximize2 className="h-4 w-4 mr-1" />
              查看详情
            </Button>
          </div>

          {!error && (
            <div className="mt-3 space-y-2">
              <div className="flex justify-between text-xs text-muted-foreground">
                <span>{progress.status}</span>
                <span>{progressValue.toFixed(1)}%</span>
              </div>
              <Progress value={progressValue} className="h-2" />
              {progress.total > 0 && (
                <div className="text-xs text-muted-foreground">
                  {formatBytes(progress.downloaded)} / {formatBytes(progress.total)}
                </div>
              )}
            </div>
          )}
        </div>
      )}

      <Dialog open={open} onOpenChange={handleClose}>
        <DialogContent className="sm:max-w-[500px]">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              {isComplete ? (
                <>
                  <CheckCircle2 className="h-5 w-5 text-blue-500" />
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
                ? 'OCR 完整版已成功安装，支持文本型和扫描版 PDF。'
                : '应用将下载 Python 运行时并安装完整版 OCR（PyMuPDF + PaddleOCR），约 270MB。'}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            {isChecking && (
              <div className="flex items-center justify-center gap-2 py-4">
                <div className="h-5 w-5 border-2 border-primary border-t-transparent rounded-full animate-spin" />
                <span className="text-sm text-muted-foreground">检查 OCR 状态...</span>
              </div>
            )}

            {!isDownloading && !isComplete && !error && (
              <div className="space-y-3">
                <div className="flex items-start gap-2 text-sm text-muted-foreground">
                  <AlertCircle className="h-4 w-4 mt-0.5 flex-shrink-0" />
                  <div>
                    <p className="font-medium mb-1">完整版 OCR 说明：</p>
                    <ul className="list-disc list-inside space-y-1 text-xs">
                      <li className="flex items-center gap-1">
                        <CheckCircle className="w-3 h-3 text-blue-600 inline flex-shrink-0" />
                        支持文本型 PDF（可复制文字的 PDF）
                      </li>
                      <li className="flex items-center gap-1">
                        <CheckCircle className="w-3 h-3 text-blue-600 inline flex-shrink-0" />
                        支持扫描版 PDF（图片型，使用 PaddleOCR 识别）
                      </li>
                      <li>包含 PyMuPDF + PaddleOCR，约 270MB</li>
                      <li>所有处理均在本地完成，数据不上传</li>
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
                    <span className="font-medium">{progressValue.toFixed(1)}%</span>
                  </div>
                  <Progress value={progressValue} className="h-2" />
                  {progress.total > 0 && (
                    <div className="text-xs text-muted-foreground text-center">
                      {formatBytes(progress.downloaded)} / {formatBytes(progress.total)}
                    </div>
                  )}
                </div>
              </div>
            )}

            {isComplete && (
              <div className="flex items-center gap-2 p-3 bg-blue-50 border border-blue-200 rounded-md">
                <CheckCircle2 className="h-5 w-5 text-blue-500 flex-shrink-0" />
                <div className="text-sm text-blue-700">
                  <p className="font-medium">完整版 OCR 已就绪</p>
                  <p className="text-xs mt-1 flex items-center gap-1">
                    <CheckCircle className="w-3 h-3 inline" />
                    支持文本型和扫描版 PDF
                  </p>
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
            {isChecking && (
              <Button disabled>
                <div className="flex items-center gap-2">
                  <div className="h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                  检查中...
                </div>
              </Button>
            )}

            {!isChecking && !isDownloading && !isComplete && (
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
              <>
                <Button variant="outline" onClick={() => onOpenChange(false)}>
                  <Minimize2 className="h-4 w-4 mr-2" />
                  后台下载
                </Button>
                <Button disabled>
                  <div className="flex items-center gap-2">
                    <div className="h-4 w-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                    下载中...
                  </div>
                </Button>
              </>
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
                <Button onClick={handleDownload}>重试</Button>
              </>
            )}
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
