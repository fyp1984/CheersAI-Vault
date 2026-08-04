# CheersAI Vault

脱敏客户端（Tauri 桌面）正式名称为 **CheersAI Vault**；企业浏览器端为 **CheersAI Vault Pro**（本仓库根目录原 Vault 前端经服务器 Runtime HTTP 适配后的客户入口）；`CheersAI Desktop` 仅指智能体工作台页面。

## 企业版部署与使用文档

企业端（浏览器 + Runtime）的操作手册与部署说明见 [`docs/enterprise/`](docs/enterprise/)：

- [`docs/enterprise/OPERATION_GUIDE.md`](docs/enterprise/OPERATION_GUIDE.md) — 面向浏览器端使用者的简易操作手册。
- [`docs/enterprise/DEPLOYMENT.md`](docs/enterprise/DEPLOYMENT.md) — 面向管理员/开发者的部署与技术说明（组件关系、环境变量、OCR/LibreOffice 安装、就绪检查与排障）。
- [`docs/enterprise/API_REFERENCE.md`](docs/enterprise/API_REFERENCE.md) — 首版四项客户 API 文档（批量提交、进度查询/失败信息、结果下载、健康检查）。

### Linux 内网客户测试部署

客户浏览器入口是本仓库根目录的原 Vault 前端（本 `vite.config.ts` 构建），为唯一客户入口。Linux 内网客户测试部署材料（systemd、Nginx、Runtime 环境变量模板、黑盒 smoke 测试脚本）见 [`deploy/linux/`](deploy/linux/)，完整拓扑与安全边界说明见 `DEPLOYMENT.md` 第 0 节与 `deploy/linux/README.md`。

本版本按**单一系统用户**交付（无账号/RBAC/多租户），仅面向企业内网使用，没有应用层鉴权；**真实 Linux 实机部署与真实 FileBay 远端闭环尚未验收**，本机 macOS 与 fake transport 结果不得当作已通过证据。
