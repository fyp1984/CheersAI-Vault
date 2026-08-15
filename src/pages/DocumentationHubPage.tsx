import { Link } from "react-router-dom";
import { ArrowRight } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { documentCatalog } from "@/lib/docs/catalog";

export default function DocumentationHubPage() {
  return (
    <div className="flex h-full flex-col bg-slate-50">
      <div className="flex-1 overflow-auto px-4 py-5 sm:px-6">
        <div className="mx-auto flex w-full max-w-7xl flex-col gap-4">
          <div className="rounded-2xl border border-sky-200 bg-sky-50/70 px-4 py-4 text-sm shadow-sm sm:px-5">
            <span className="font-medium text-slate-900">
              <a
                href="https://dcnd0q32i5v3.feishu.cn/wiki/TVChw3onji9mVdkx96tcXsSYnlf"
                target="_blank"
                rel="noreferrer"
                className="underline underline-offset-4 hover:text-sky-700"
              >
                CheersAI Desktop 注册与使用手册
              </a>
            </span>
          </div>

          <div className="grid gap-4 xl:grid-cols-2">
            {documentCatalog.map((item) => (
              <Card key={item.id} className="border-slate-200 shadow-sm transition-shadow hover:shadow-lg">
                <CardHeader className="space-y-4">
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div className="space-y-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <Badge
                          variant="outline"
                          className={
                            item.id === "personal"
                              ? "border-emerald-200 bg-emerald-50 text-emerald-700"
                              : "border-blue-200 bg-blue-50 text-blue-700"
                          }
                        >
                          {item.versionLabel}
                        </Badge>
                        <Badge variant="outline" className="border-slate-200 bg-slate-50 text-slate-600">
                          正式文档入口
                        </Badge>
                      </div>
                      <CardTitle className="text-2xl text-slate-950">{item.title}</CardTitle>
                    </div>
                    <Button asChild size="sm" className="rounded-full">
                      <Link to={`/docs/${item.id}`}>
                        开始阅读
                        <ArrowRight className="h-4 w-4" />
                      </Link>
                    </Button>
                  </div>
                </CardHeader>

                <CardContent>
                  <div className="space-y-3 rounded-2xl border border-slate-200 bg-white p-4">
                    <div className="text-sm font-semibold text-slate-900">操作步骤与注意事项</div>
                    <div className="space-y-2">
                      {item.quickLinks.map((link) => (
                        <Link
                          key={link.section}
                          to={`/docs/${item.id}?section=${encodeURIComponent(link.section)}`}
                          className="flex items-center justify-between rounded-xl border border-slate-200 px-3 py-2 text-sm text-slate-700 transition-colors hover:border-blue-200 hover:bg-blue-50 hover:text-blue-700"
                        >
                          <span>{link.label}</span>
                          <ArrowRight className="h-4 w-4" />
                        </Link>
                      ))}
                    </div>
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
