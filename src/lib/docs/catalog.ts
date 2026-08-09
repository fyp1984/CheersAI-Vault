import personalGuideRaw from "../../../docs/USER_GUIDE.md?raw";
import enterpriseGuideRaw from "../../../docs/enterprise/OPERATION_GUIDE.md?raw";

export type DocumentId = "personal" | "enterprise";

export interface DocumentQuickLink {
  label: string;
  section: string;
}

export interface DocumentCatalogItem {
  id: DocumentId;
  title: string;
  versionLabel: string;
  quickLinks: DocumentQuickLink[];
  content: string;
}

function collectTopLevelSections(content: string): Array<{ heading: string; body: string }> {
  const lines = content.replace(/\r\n/g, "\n").split("\n");
  const sections: Array<{ heading: string; body: string }> = [];
  let currentHeading = "";
  let currentLines: string[] = [];

  const pushCurrent = () => {
    if (!currentHeading) {
      return;
    }
    sections.push({
      heading: currentHeading,
      body: [currentHeading, ...currentLines].join("\n").trim(),
    });
  };

  for (const line of lines) {
    if (line.startsWith("## ")) {
      pushCurrent();
      currentHeading = line.trim();
      currentLines = [];
      continue;
    }

    if (currentHeading) {
      currentLines.push(line);
    }
  }

  pushCurrent();

  return sections;
}

function pickSections(content: string, selectedHeadings: string[]): string {
  const sections = collectTopLevelSections(content);
  return sections
    .filter((section) => selectedHeadings.includes(section.heading))
    .map((section) => section.body)
    .join("\n\n")
    .trim();
}

const personalContent = pickSections(personalGuideRaw, [
  "## 6. 核心功能操作手册",
  "## 7. 故障排查手册",
]);

const enterpriseContent = pickSections(enterpriseGuideRaw, [
  "## 1. 打开企业端",
  "## 2. 提交文件（先预览、再确认）",
  "## 3. 查看结果",
  "## 4. 下载与恢复",
  "## 5. 操作日志：搜索与筛选",
  "## 5.1 规则与敏感词（「规则配置」页）",
  "## 5.2 沙箱/PIN（「沙箱管理」页）",
  "## 6. 支持格式与限制",
  "## 7. 扫描件（图片型）PDF",
  "## 8. 常见错误码",
  "## 8.1 运行时异常的白话处理",
  "## 9. 上传脱敏结果到 FileBay（服务器管理的私有仓库）",
  "## 11. 企业版文档入口索引（技术人员使用）",
  "## 12. API 接入文档入口（系统对接工程师）",
  "## 13. 私有化部署与运维文档入口（部署管理员）",
]);

export const documentCatalog: DocumentCatalogItem[] = [
  {
    id: "personal",
    title: "个人版操作文档",
    versionLabel: "个人版",
    quickLinks: [
      { label: "文件脱敏", section: "62-文件脱敏" },
      { label: "文件反脱敏", section: "63-文件反脱敏" },
      { label: "规则与敏感词库", section: "64-规则与敏感词库" },
      { label: "FileBay 上传", section: "66-filebay-上传" },
      { label: "故障排查", section: "7-故障排查手册" },
    ],
    content: personalContent,
  },
  {
    id: "enterprise",
    title: "企业版操作文档",
    versionLabel: "企业版",
    quickLinks: [
      { label: "打开企业端", section: "1-打开企业端" },
      { label: "提交文件", section: "2-提交文件先预览再确认" },
      { label: "下载与恢复", section: "4-下载与恢复" },
      { label: "常见错误码", section: "8-常见错误码" },
      { label: "上传到 FileBay", section: "9-上传脱敏结果到-filebay服务器管理的私有仓库" },
      { label: "文档入口索引", section: "11-企业版文档入口索引技术人员使用" },
      { label: "API 接入入口", section: "12-api-接入文档入口系统对接工程师" },
      { label: "私有化部署入口", section: "13-私有化部署与运维文档入口部署管理员" },
    ],
    content: enterpriseContent,
  },
];

export function getDocumentById(id: string | undefined): DocumentCatalogItem | undefined {
  return documentCatalog.find((item) => item.id === id);
}
