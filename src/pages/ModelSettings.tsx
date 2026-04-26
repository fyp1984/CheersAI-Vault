import { PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import { BrainCircuit } from "lucide-react";

export default function ModelSettings() {
  return (
    <div className="flex flex-col h-full">
      <PageHeader
        title="模型设置"
        description="配置 AI 模型参数（实验性功能）"
      />
      <div className="flex-1 overflow-auto p-6">
        <Card className="max-w-lg">
          <CardHeader>
            <CardTitle className="text-base flex items-center gap-2">
              <BrainCircuit className="w-5 h-5 text-indigo-500" />
              AI 辅助识别
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1.5">
              <Label>API 端点</Label>
              <Input placeholder="https://api.example.com/v1" />
            </div>
            <div className="space-y-1.5">
              <Label>API Key</Label>
              <Input type="password" placeholder="sk-..." />
            </div>
            <div className="space-y-1.5">
              <Label>模型名称</Label>
              <Input placeholder="gpt-4o-mini" />
            </div>
            <Button className="bg-indigo-500 hover:bg-indigo-600 w-full">
              保存配置
            </Button>
            <p className="text-xs text-gray-400 text-center">
              注意：所有数据处理均在本地完成，AI 功能仅用于规则生成辅助
            </p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
