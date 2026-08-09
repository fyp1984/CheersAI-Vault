import { useEffect, useMemo } from "react";
import { Link, Navigate, useParams, useSearchParams } from "react-router-dom";
import { ArrowLeft, ChevronRight } from "lucide-react";
import { MarkdownArticle, extractDocumentHeadings } from "@/components/docs/MarkdownArticle";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { getDocumentById } from "@/lib/docs/catalog";

export default function DocumentationViewerPage() {
  const { docId } = useParams();
  const [searchParams] = useSearchParams();
  const docEntry = getDocumentById(docId);
  const requestedSection = searchParams.get("section");

  const headings = useMemo(() => {
    if (!docEntry) {
      return [];
    }
    return extractDocumentHeadings(docEntry.content);
  }, [docEntry]);

  useEffect(() => {
    if (!requestedSection) {
      return;
    }

    const timerId = window.setTimeout(() => {
      window.document.getElementById(requestedSection)?.scrollIntoView({ behavior: "smooth", block: "start" });
    }, 60);

    return () => window.clearTimeout(timerId);
  }, [requestedSection]);

  if (!docEntry) {
    return <Navigate to="/docs" replace />;
  }

  return (
    <div className="flex h-full flex-col bg-slate-50">
      <div className="flex-1 overflow-hidden px-4 py-5 sm:px-6">
        <div className="mx-auto grid h-full w-full max-w-7xl gap-5 xl:grid-cols-[320px_minmax(0,1fr)]">
          <div className="flex min-h-0 flex-col gap-5">
            <Button asChild variant="outline" size="sm" className="w-fit">
              <Link to="/docs">
                <ArrowLeft className="h-4 w-4" />
                返回索引
              </Link>
            </Button>

            <Card className="border-slate-200 shadow-sm">
              <CardHeader className="space-y-3">
                <div className="flex flex-wrap items-center gap-2">
                  <Badge
                    variant="outline"
                    className={
                      docEntry.id === "personal"
                        ? "border-emerald-200 bg-emerald-50 text-emerald-700"
                        : "border-blue-200 bg-blue-50 text-blue-700"
                    }
                  >
                    {docEntry.versionLabel}
                  </Badge>
                  <Badge variant="outline" className="border-slate-200 bg-slate-50 text-slate-600">
                    应用内阅读
                  </Badge>
                </div>
                <CardTitle className="text-2xl text-slate-950">{docEntry.title}</CardTitle>
              </CardHeader>
            </Card>

            <Card className="border-slate-200 shadow-sm">
              <CardHeader>
                <CardTitle className="text-base text-slate-950">目录导航</CardTitle>
              </CardHeader>
              <CardContent>
                <ScrollArea className="h-[240px] pr-3 xl:h-[calc(100vh-420px)]">
                  <div className="space-y-1">
                    {headings.map((heading) => (
                      <Link
                        key={heading.id}
                        to={`/docs/${docEntry.id}?section=${encodeURIComponent(heading.id)}`}
                        className={`flex items-center gap-2 rounded-xl px-3 py-2 text-sm transition-colors ${
                          requestedSection === heading.id
                            ? "bg-blue-50 text-blue-700"
                            : "text-slate-600 hover:bg-slate-100 hover:text-slate-900"
                        } ${heading.level === 3 ? "ml-4" : ""}`}
                      >
                        <ChevronRight className="h-4 w-4 shrink-0 text-slate-400" />
                        <span className="min-w-0 flex-1 truncate">{heading.text}</span>
                      </Link>
                    ))}
                  </div>
                </ScrollArea>
              </CardContent>
            </Card>
          </div>

          <Card className="min-h-0 border-slate-200 shadow-sm">
            <CardContent className="h-full p-0">
              <ScrollArea className="h-full">
                <div className="mx-auto w-full max-w-4xl px-5 py-6 sm:px-8">
                  <MarkdownArticle markdown={docEntry.content} />
                </div>
              </ScrollArea>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
