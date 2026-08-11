# 文件名敏感信息脱敏问题根因分析与修复说明

## 1. 问题摘要

本次排查覆盖 CheersAI Vault 个人版桌面端与企业版 Runtime 两条链路，问题集中在“文件名敏感信息脱敏”与“手工替换后文件名不同步”两个方向：

- 桌面端：手工替换正文后，`masked_file_stem` / `masked_file_name` 未稳定同步，导致导出文件名仍可能保留旧脱敏结果。
- 企业版：文件名脱敏沿用了正文占位符风格，输出可能为 `姓名1`、`***PHONE***1`，不符合文件名合规脱敏规则。
- 双端共性：文件名清洗逻辑把 `*` 也当作非法字符替换，导致即使前序逻辑产出了 `张*`、`138****5678`，最终落盘名称仍退化为 `张_`、`138____5678`。

## 2. 根因定位

### 根因 A：文件名复用了正文占位符脱敏策略

旧逻辑直接沿用正文 `MaskingSession` / `mask_value_with_ner` 的输出语义，适合正文映射，不适合文件名合规展示。

直接后果：

- 姓名被替换为 `姓名1`，而不是 `张*`
- 手机号被替换为 `***PHONE***1`，而不是 `138****5678`
- 身份证、银行卡同理只得到占位符，不满足“保留头尾”的法规口径

### 根因 B：桌面端文件名姓名识别缺少文件名专用 findings

桌面端旧实现对文件名仍使用正文式 NER 检测。文件名通常缺少“姓名：张三”这类上下文，导致 `chinese_name` 在文件名里不稳定命中。

直接后果：

- 桌面端 `张三-13900000000-报价单.md` 可识别手机号，但姓名可能漏脱敏

### 根因 C：手工替换逻辑对“仅文件名命中”的场景提前返回

`applyManualReplacementToPreview()` 旧逻辑只有正文命中时才继续处理，纯文件名映射替换会在 `count === 0` 时提前返回。

直接后果：

- 用户手工把 `13812345678` 替换为“联系电话”时，正文无变化则文件名也不会更新

### 根因 D：文件名安全清洗误杀脱敏星号

桌面端 `sanitize_output_file_stem`、企业版 `sanitize_display_name`、前端 `safeStem` / `sanitizeMaskedFileStem` 都把 `*` 当作非法字符替换为 `_`。

直接后果：

- 实际结果从 `张*` 退化成 `张_`
- `138****5678` 退化成 `138____5678`

### 根因 E：身份证规则未覆盖 15 位号码

共享内核和桌面 NER 的身份证正则最初仅覆盖 18 位居民身份证号，没有纳入 15 位旧身份证格式。

直接后果：

- 15 位身份证文件名场景出现漏脱敏

## 3. 修复方案

### 3.1 共享内核新增文件名专用脱敏能力

在 `src-tauri/crates/engine-core/src/lib.rs` 中新增：

- `FilenameMaskingResult`
- `collect_filename_findings(...)`
- `mask_filename(...)`

并引入文件名专用脱敏规则：

- 姓名：保留首字，其余替换为 `*`
- 手机号：保留前 3 后 4
- 身份证号：保留前 6 后 4
- 银行卡号：保留前 4 后 4
- 邮箱：保留本地名首字符
- 护照、IPv4、敏感词库：分别按文件名场景单独处理

### 3.2 桌面端切换到共享文件名脱敏链路

在 `src-tauri/src/commands/masking.rs` 中：

- `build_masked_file_stem(...)` 改为调用共享 `mask_filename(...)`
- 文件名 findings 改为“文件名专用 findings + 原有 deterministic findings”合并
- `preview_masking(...)` 新增 `original_file_stem` / `original_file_name`
- `save_preview_result(...)` 回退计算文件名时也统一走新链路

### 3.3 企业版 Runtime 统一复用共享内核

在 `apps/vault-runtime-api/src/processing.rs` 中：

- `mask_display_name(...)` 改为直接调用共享 `collect_filename_findings(...) + mask_filename(...)`
- 敏感词快照存在时，无需再依赖额外页面开关，也会参与同一轮文件名脱敏

### 3.4 手工替换联动文件名

在 `src/components/file/maskingPreviewManualReplace.ts` 中：

- 允许“仅文件名命中”继续处理，不再因正文替换计数为 0 提前返回
- 同步更新 `mapping`、`masked_file_stem`、`masked_file_name`
- 保证预览确认后，导出文件名与最终保存内容一致

### 3.5 保留脱敏星号

在以下位置放宽文件名清洗规则，保留脱敏用 `*`：

- `src-tauri/src/commands/masking.rs`
- `apps/vault-runtime-api/src/lib.rs`
- `src/lib/runtime/downloadName.ts`
- `src/components/file/maskingPreviewManualReplace.ts`

### 3.6 补齐 15 位身份证支持

更新位置：

- `src-tauri/crates/engine-core/src/lib.rs`
- `src-tauri/src/core/ner.rs`
- `src/store/ruleStore.ts`

## 4. 验证结论

已通过的关键验证包括：

- 前端单测：`26/26`
- 个人版桌面 Rust 测试：`24/24`
- 企业版 Runtime Rust 测试：`165/165`
- 共享内核新增边界测试全部通过

已覆盖的重点场景：

- 姓名、手机号、18 位身份证、15 位身份证、银行卡、敏感词库
- 文件名开头 / 中间 / 结尾
- 无分隔拼接
- 数字重叠场景（手机号嵌在银行卡长串中）
- 手工替换后文件名同步
- 企业版权限与个人版基础规则各自独立生效

## 5. 仍需说明的事项

- 本轮已完成代码级、单元级、Runtime 级回归验证。
- 桌面 UI 截图留存仍需在安装包人工验收环节补录；此前受 macOS 桌面自动化权限限制，无法在当前环境稳定自动采集原生窗口操作截图。
- 当前环境可产出内部测试用 unsigned / ad-hoc DMG，不代表 Apple 正式签名公证包。
