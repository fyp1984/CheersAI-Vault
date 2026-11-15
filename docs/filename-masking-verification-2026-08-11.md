# 文件名脱敏修复验证记录

## 1. 验证范围

- 个人版桌面端文件名脱敏
- 企业版 Runtime 文件名脱敏
- 手工替换后文件名联动
- 版本兼容相关下载名生成

## 2. 自动化验证命令

```bash
corepack pnpm test:unit
corepack pnpm exec tsc --noEmit
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path apps/vault-runtime-api/Cargo.toml
cargo test --manifest-path src-tauri/crates/engine-core/Cargo.toml
```

## 3. 结果摘要

- `corepack pnpm test:unit`：通过，`26/26`
- `corepack pnpm exec tsc --noEmit`：通过
- `cargo test --manifest-path src-tauri/Cargo.toml`：通过，`24/24`
- `cargo test --manifest-path apps/vault-runtime-api/Cargo.toml`：通过，`165/165`，`1 ignored`
- `cargo test --manifest-path src-tauri/crates/engine-core/Cargo.toml`：新增文件名边界测试通过

## 4. 重点覆盖场景

- 文件名姓名脱敏：`张三-报价单.md -> 张*-报价单_脱敏.md`
- 手机号脱敏：`13812345678 -> 138****5678`
- 18 位身份证：保留前 6 后 4
- 15 位身份证：保留前 6 后 4
- 银行卡号：保留前 4 后 4
- 敏感词库：`机密项目 -> [机密]`
- 无分隔拼接：`张三13812345678合同`
- 重叠数字串：银行卡长串中不误切手机号
- 手工替换：正文不变时，文件名替换仍生效
- 下载名：保留脱敏星号，不再被 `_` 覆盖

## 5. 跨版本结论

- 个人版：通过桌面 Rust 命令层与手工替换纯函数测试验证
- 企业版：通过 Runtime 全量回归、批量创建、预览创建、下载接口验证
- 双端共享内核已统一到 `engine_core::mask_filename(...)`

## 6. 待人工验收项

- 安装包内桌面 UI 实际点击截图
- 旧版本客户端对新版本 `0.1.41` 的自动检测提示
- 手工触发更新链路与版本展示核对
