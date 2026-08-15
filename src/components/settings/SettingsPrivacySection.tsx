/* Copyright 2026 CheersAI Team. Licensed under Apache-2.0 */
import { ShieldCheck, AlertTriangle, Info } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Message } from "@/components/ui/cheersai-ui";
import { useExcelMaskingStore } from "@/store/excelMaskingStore";
import { RETAIN_MESSAGES } from "@/components/file/ExcelMaskingDialog";

export default function SettingsPrivacySection() {
  const { privacy, setPrivacy } = useExcelMaskingStore();

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <ShieldCheck className="w-5 h-5 text-blue-500" />
          隐私与 Excel 自动脱敏
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-5">
        <div className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <div className="flex-1">
              <Label className="text-sm font-medium text-gray-900">
                上传 Excel 自动弹脱敏对话框
              </Label>
              <p className="text-xs text-gray-500 mt-0.5">
                关闭后需手动打开；支持 .xlsx / .xls / .xlsm / .csv。
              </p>
            </div>
            <Switch
              checked={privacy.excelAutoMaskDialog}
              onCheckedChange={(v) =>
                setPrivacy({ excelAutoMaskDialog: Boolean(v) })
              }
            />
          </div>

          <div className="flex items-center justify-between gap-4">
            <div className="flex-1">
              <Label className="text-sm font-medium text-gray-900">
                默认预选「保留加密源」
              </Label>
              <p className="text-xs text-gray-500 mt-0.5">
                开启后 Tab0 Checkbox 预选 true，但仍需用户点一下显式同意（合规不跳过勾选）。
              </p>
            </div>
            <Switch
              checked={privacy.excelDefaultRetainEncryptedSource}
              onCheckedChange={(v) =>
                setPrivacy({ excelDefaultRetainEncryptedSource: Boolean(v) })
              }
            />
          </div>
        </div>

        <div className="space-y-3 pt-1">
          <Message type="info" title="三提示文案 · 三处复用">
            <div className="space-y-2 text-xs leading-6">
              <div className="flex items-start gap-2">
                <Info className="w-4 h-4 mt-0.5 shrink-0 text-blue-600" />
                <div>
                  <b>1) 对话框内：</b>Tab0 底部提示，用于预选与告知勾选含义。
                </div>
              </div>
              <div className="flex items-start gap-2">
                <Info className="w-4 h-4 mt-0.5 shrink-0 text-blue-600" />
                <div>
                  <b>2) 执行前：</b>预览 Tab 底部 Confirm 二次提示，需显式勾选。
                </div>
              </div>
              <div className="flex items-start gap-2">
                <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0 text-amber-600" />
                <div>
                  <b>3) 反脱敏缺材料时：</b>FileUnmask 缺少 encrypted_source 路径 A 时的琥珀色警告。
                </div>
              </div>
            </div>
          </Message>

          <Message type="warning" title="统一文案（三处完全一致）">
            <p className="text-xs leading-6">{RETAIN_MESSAGES.tab0}</p>
          </Message>
        </div>
      </CardContent>
    </Card>
  );
}
