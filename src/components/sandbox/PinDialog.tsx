import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";

interface PinDialogProps {
  open: boolean;
  onClose: () => void;
  onSubmit: (pin: string) => Promise<void>;
  title?: string;
}

export function PinDialog({
  open,
  onClose,
  onSubmit,
  title = "输入沙箱 PIN",
}: PinDialogProps) {
  const [pin, setPin] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const handleSubmit = async () => {
    if (pin.length < 4) {
      setError("PIN 至少 4 位");
      return;
    }
    setLoading(true);
    setError("");
    try {
      await onSubmit(pin);
      setPin("");
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          <Label htmlFor="pin">PIN 码</Label>
          <Input
            id="pin"
            type="password"
            value={pin}
            onChange={(e) => setPin(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
            placeholder="请输入 PIN"
            maxLength={12}
          />
          {error && <p className="text-sm text-red-500">{error}</p>}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            取消
          </Button>
          <Button onClick={handleSubmit} disabled={loading}>
            {loading ? "验证中..." : "确认"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
