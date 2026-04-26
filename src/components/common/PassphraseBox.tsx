import { useState } from "react";
import { Eye, EyeOff, RefreshCw, Copy, X } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { tauriCommands } from "@/lib/tauri";

interface PassphraseBoxProps {
  value: string;
  onChange: (v: string) => void;
  label?: string;
}

export function PassphraseBox({
  value,
  onChange,
  label = "口令",
}: PassphraseBoxProps) {
  const [show, setShow] = useState(false);
  const [copied, setCopied] = useState(false);

  const generate = async () => {
    try {
      const p = await tauriCommands.generatePassphrase();
      onChange(p);
    } catch {
      // not implemented yet — show placeholder
      onChange("xxxx-xxxx-xxxx-xxxx");
    }
  };

  const copy = () => {
    navigator.clipboard.writeText(value);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const clear = () => {
    onChange("");
  };

  return (
    <div className="space-y-1.5">
      <Label>{label}</Label>
      <div className="flex gap-2">
        <div className="relative flex-1">
          <Input
            type={show ? "text" : "password"}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder="输入或生成口令（可选，会自动保存）"
            className="pr-10"
          />
          <button
            type="button"
            onClick={() => setShow(!show)}
            className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600"
          >
            {show ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
          </button>
        </div>
        <Button variant="outline" size="icon" onClick={generate} title="生成随机口令">
          <RefreshCw className="w-4 h-4" />
        </Button>
        <Button
          variant="outline"
          size="icon"
          onClick={copy}
          disabled={!value}
          title={copied ? "已复制" : "复制口令"}
        >
          <Copy className="w-4 h-4" />
        </Button>
        <Button
          variant="outline"
          size="icon"
          onClick={clear}
          disabled={!value}
          title="清除口令"
        >
          <X className="w-4 h-4" />
        </Button>
      </div>
      {value && (
        <p className="text-xs text-green-600">
          口令已保存，下次使用时会自动填充
        </p>
      )}
    </div>
  );
}
