# CODE REVIEW CHECKLIST - CheersAI-SDLC 代码审查清单 (Gate 3 补充)
> 用于 CD 发起 PR/MR 后，审查者 100% 勾选后才允许 merge

## 一、功能 & 设计对齐
- [ ] 实现完全覆盖 PRD 中 Must/P0 AC
- [ ] 没有偏离 DESIGN.md 中的模块拆分 / 接口契约
- [ ] 没有偷偷引入 DESIGN 未列出的新依赖/新框架

## 二、代码规范 & 可读性
- [ ] 已执行对应语言的 fmt/lint（Go fmt / TS prettier+eslint / Java checkstyle / Python ruff / Rust rustfmt）
- [ ] 无硬编码密钥、JWT、密码（若出现直接 BLOCK，零容忍）
- [ ] 命名符合单仓约定（例: Go/PascalCase 导出，TS/React PascalCase 组件）
- [ ] 函数单一职责，单函数 ≤ 120 行（复杂逻辑提取 helper）
- [ ] 新建 .go/.ts/.java/.py/.rs 文件含当前年版权头

## 三、测试覆盖
- [ ] 新增 / 修改逻辑对应单测 **正向 + 异常**均有
- [ ] 覆盖率 ≥ 对应语言阈值（Go 70% / TS 70% / Java 70% / Python 80% / Rust 70%）
- [ ] 没有「只改 happy path 不改 error path」的情况

## 四、安全 & 合规
- [ ] C3/PII 字段：不出现在 INFO/DEBUG 日志；展示默认脱敏
- [ ] C4 机密：使用 KMS / env-var / secret-manager，严禁 commit 明文
- [ ] 所有新增 HTTP 路由鉴权校验（public 路由需显式声明 allowlist）
- [ ] 无 SQL 拼接 / XSS 注入 / path 穿越风险（参数化查询 / 路径校验）

## 五、性能 & 资源
- [ ] 无 N+1 查询（DB 批量 or join）
- [ ] 大文件/大 JSON 采用流式处理（避免一次性全部加载到内存）
- [ ] 循环中无重复同步 IO

## 六、Git & 分支
- [ ] 分支符合 Gitflow：`feature/<ticket>-…`（或 `hotfix/…`）
- [ ] 无 「WIP / fix / temp」 这种 commit；commit message 符合 conventional commit
- [ ] 无 trailing whitespace / 行尾空格
- [ ] go.mod / package.json / Cargo.toml 变动对应 `make tidy` / `pnpm install` / `cargo update` 执行过

## 七、签名
- 审查者 (name/email): ________________
- 审查日期: ____
- 最终结论: ✅ APPROVE / ❌ CHANGES REQUESTED / ⏳ COMMENT
