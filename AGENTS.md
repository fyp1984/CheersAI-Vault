# AGENTS.md · CheersAI-Vault 个人版 开发守则

> 生效日期: 2026-08-14
> 项目根: [CheersAI-Vault/](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault)
> 路由清单来源: [App.tsx](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/App.tsx#L34-L93)

## 一、项目结构
```
src/              # React 18 + Vite 5 前端 (TS)
src-tauri/        # Rust 后端 (Runtime 侧若有)
index.html        # HashRouter HTML 入口
package.json      # pnpm workspace 单包
filebay-config.json  # FileBay 侧配置（不强制）
```

## 二、技术栈强制
- **前端**：React 18 + TypeScript + Vite 5 + TailwindCSS 3 + shadcn/ui + lucide-react
- **后端**：Rust （Tauri 或 Runtime）；禁止在此项目引入 Vue/Spring/NestJS 等重型异类框架
- **测试**：Vitest + Playwright (如启用)

## 三、编码前必跑命令
```bash
cat package.json              # 查看现有命令，不要另起炉灶
cat ../sdlc/AGENTS.md         # 必须先读企业级流程守则
```

### 提交前 5 条必执行（与 ../sdlc/gates/G3-CODE.spec.yaml 对齐）
1. `pnpm install` 若 `package.json` 或 lock 改动
2. `pnpm lint` (如有；若无则 `eslint src --ext .ts,.tsx`)
3. `pnpm typecheck` 或 `pnpm tsc --noEmit`
4. `pnpm test` / `pnpm vitest run`
5. **去掉尾行空格**（所有 .ts/.tsx/.md/.json）

## 四、核心业务契约锚点
- 脱敏：[lib/runtime/client.ts](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/lib/runtime/client.ts#L157-L355)
- 支持格式投影：[lib/runtime/formatCatalog.ts](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/lib/runtime/formatCatalog.ts#L10-L29)
- 规则类型：[types/rules.ts](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/types/rules.ts)
- 反脱敏 UI 安全文案：[pages/FileUnmaskBrowser.tsx](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/CheersAI-Vault/src/pages/FileUnmaskBrowser.tsx#L44-L62)

## 五、企业级全流程协作（硬约束）
🔗 企业级全流程协作（Harness Engineering × SDLC-Gatekeeper）：执行任何需求/设计/编码/测试/发布前，必须先阅读并严格遵守 [CheersAI-SDLC /sdlc/AGENTS.md](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/sdlc/AGENTS.md) 与 [sdlc/README.md](file:///Users/sevensimle/Documents/WorkSpace/CheersAI/sdlc/README.md)；所有交接须经 G1-G5 Harness 脚本 + verify-harness-integrity (HE-2) + 不可篡改链式审计 (HE-3)。严禁脱离 Harness Engineering SoD 一人兼任 RA/TD/CD/QA/RO 任意两岗组合（RA+TD/TD+CD/CD+QA/QA+RO 4 种必 FAIL）。
