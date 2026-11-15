# PRD - Vault Excel 文件脱敏增强功能

> 编号: PRD-IMP-SDLCVAULT002 | 版本: v1.3（基于 v1.2 最小增量） | 负责人 (RA): sdlc-ra-002@cheersai.ai | 修订日期: 2026-08-27
> 对齐 BASELINE: 需求 v1.2（6 条基线冻结，含沙箱口令复用 + 样式保真 + 加密留存三模式）
> **v1.3 状态：当前 Excel P0 实施与验收基线。本版本以 v1.2 全文为底稿做最小增量修改；后续如负责人调整细节，应通过新版本继续维护，不直接改写历史版本。所有实质差异见文末“附录 B：v1.2 → v1.3 变更清单”。**

## 一、背景与目标

### 1.1 业务背景

CheersAI-Vault 是 CheersAI「个人隐私安全屋 + 企业脱敏平台」的核心桌面端产品，已支持 Markdown、PDF、Word、PPT、CSV、Excel 等多种文档格式的 NER 自动识别、规则脱敏、映射反脱敏（.cmap）能力。用户在合规场景（客户名单出数、HR 员工信息导出、法务合同数据交换、数据分析师样本提供）中，Excel 是使用占比最高的结构化载体。以下是现有实现的业务堵点：

1. Excel 脱敏只支持「自动识别 + 值级掩码」，无法精准定位到某一行、某一列或任意单元格区域，敏感信息过度脱敏（错掩码非敏感列）或不足（漏掩码 PII）都会出现在实际使用中。
2. 输出格式默认转为 Markdown，会丢失样式、图表、公式、批注、数据验证、命名范围、合并单元格、嵌入式对象。分析师和法务收到脱敏文件后无法直接交付，需要人工二次整理，导致客户满意度下降。
3. 反脱敏依赖「用户自行保管原件 + .cmap」，但用户常常丢失原件或混存，合规场景（如审计抽查需还原当月样本）无法还原，需要全量重新生成文件，风险不可控。
4. 映射文件（.cmap）与加密留存源文件（如有）使用两套口令或完全无口令，需要用户记忆新密码；体验差、记忆负担重，结果就是「干脆不设密码」或「密码设太弱」，合规 C4 级 PII（身份证号/手机号/银行卡）在磁盘泄露时存在明文风险。

### 1.2 目标（可量化 OKR）

1. O：让 Excel 脱敏能力成为 Vault 的头号合规能力（个人版开箱即用、企业版策略模板化交付），在 PII 密集场景替代人工 Excel 掩码操作。
2. KR1：Excel 字段级 + 单元格级脱敏命中率（用户期望掩码的单元格实际被掩码）≥ 99.5%；随机抽 20 份真实业务 Excel 样本人工标注对比。
3. KR2：样式保真度（脱敏后文件与原文件视觉一致率）≥ 99%，允许 OOXML 内部命名空间或属性顺序差异；公式保留率 100%；图表不空白率 100%。
4. KR3：反脱敏成功率：A 路径（.ecmap + .encrypted_src）= 100%；B 路径（.ecmap + 用户原件）= 100%；材料缺失时不发生猜测还原且有明确指引。
5. KR4：用户体验：同一次「加密留存源文件」操作，无需额外设置新口令的比例（复用沙箱口令占比）≥ 80%（埋点统计）。
6. KR5：性能：10 万行 × 20 列（中等规模客户名单）从上传 → 对话框 → 产出脱敏 xlsx + .ecmap + 可选加密源文件，端到端 ≤ 120 秒；50 行预览 ≤ 2 秒。

### 1.3 非目标（Non-Goals / Out of Scope）

1. 非目标 1：Parquet、JSON Lines、ODS、XLSB 等其他结构化格式。
2. 非目标 2：VBA 宏代码执行保留（.xlsm 保留宏代码但不执行宏，只保留文件中的 OOXML Macro 流；宏加密或签名不保证）。
3. 非目标 3：云端多人协同编辑 Excel 的实时脱敏。
4. 非目标 4：Excel 外部数据源（Query / PowerPivot）链接的刷新，仅保留定义不主动刷新。
5. 非目标 5：将 .cmap 旧格式强制升级为 .ecmap；两者共存且向后兼容。
6. **非目标 6（v1.3 新增，见附录 B）**：`.xlsm` 不在当前 P0 承诺支持的输入格式范围内（P0 输入格式为 `.xlsx`/`.xls`/`.csv`，见 §3.4 兼容性验收）；上传 `.xlsm` 时产品尽力而为处理，处理结果不作为 P0 验收阻断项，呼应非目标 2"不保证宏保留"的既有表述，只是把"是否属于 P0 输入格式"讲清楚。

### 1.4 合规要求

1. 本 PRD 涉及最高数据分级：C4（身份证号、真实姓名、手机号、银行卡号、邮箱、家庭住址为 PII 密级字段）。
2. 涉及 PII 字段清单：真实姓名、手机号、身份证号（15/18 位）、邮箱地址、银行卡号、家庭住址、公司统一社会信用代码（合规证件类 COMPLIANCE_ID 预留）。
3. 所有日志（前端埋点、后端 processing_history、stdout/stderr）严禁出现 PII 明文；允许出现 SHA-256 前 12 位短哈希 + 脱敏展示前缀（如「张*」「139****」）。

## 二、用户故事与核心场景

### 2.1 用户画像

| 角色 | 描述 | 痛点 |
|---|---|---|
| 个人版 - HR 专员小 A | 在中小企业负责员工招聘与员工信息维护；导出候选人面试名单（Excel）给到面试官，需隐藏手机号/身份证/住址 | 旧版转 Markdown 丢失公司 logo 条件格式与筛选；面试官习惯用 Excel 筛选排序，回去还要重新整理 |
| 个人版 - 数据分析师小 B | 对外提交数据样本（含用户属性与行为列）给乙方分析公司；需隐藏个人属性只留 ID + 行为列 | 纯规则脱敏会把列中某一行「内部备注：手机号见张三」这类半结构化信息也误杀；需要单元格精修 |
| 企业版 - 合规经理小 C | 每月对 10+ 份客户名单做同样脱敏；需要把策略固化为模板，下一月一键套用，审计时还原当月原始样本 | 目前没有模板，每月要重新点 20 多列；审计还原经常找不到原件 |
| 企业版 - 安全管理员小 D | 要满足 ISO 27001「加密存储 + 可追溯还原」条款；不希望用户单独记太多密码；密码策略要求高 | 用户常把密码记在便利贴或同一份 Excel 第一行；复用沙箱口令能降低密码泄露面 |

### 2.2 用户故事（As a ... I want ... so that ...）

- US-01：作为 HR 小 A，上传候选人名单 Excel 后立即弹出脱敏对话框，系统自动识别「姓名、手机、身份证、邮箱」四列，我直接点执行就能保留原条件格式与图表，这样我发给面试官不需要再做二次美化。
- US-02：作为分析师小 B，自动识别后的基础列我要保持，但第 15 行（VIP 用户）的备注单元格要单独隐藏，第 50-200 行整行清空，所以需要单元格级的区域指定。
- US-03：作为合规经理小 C，我把「客户名单」字段 → 策略 映射保存为模板 `客户名单_2026_Q3.json`，下次一键应用，避免重复勾选，节省每月 4 小时人工配置。
- US-04：作为管理员小 D，我要求「勾选加密留存」时默认复用沙箱口令（.cmap 已经在使用的密码），且无法用这组密码跨文件互解，既降低用户记忆负担又不牺牲域隔离安全。
- US-05：作为任何用户，我勾选加密留存时必须明确告知「不勾选就无法仅凭 .ecmap 还原，请自行持有原件」，避免未来合规审计时被误导。
- US-06：作为用户，我希望在设置里有开关，关掉自动弹 Excel 对话框（在处理一堆非 Excel 时不被干扰），下次需要 Excel 再打开即可。

### 2.3 核心业务流程

```
[用户上传 .xlsx/.xls/.csv]
    ↓ detect_format = Excel/CSV
[读 settings.privacy.excelAutoMaskDialog]
    ↓ false: 走旧 FileProcessDesktop 流程（preview_masking）
    ↓ true:  打开 ExcelMaskingDialog（4 Tab）
        Tab0「合规与留存」
            提示 + 勾选☑加密留存
            三 radio 模式：①复用沙箱口令（默认）②单独口令 ③设备密钥绑定
            联动：sandbox.passphrase 为空时 ①禁用，自动 fallback 到 ②
        Tab1「字段配置」
            autoDetect + 用户手动逐列选：字段类型 + 策略（M-03/M-05/M-06/M-07 + 4 预留灰态）
            全选/反选 + 企业版导入/导出模板
        Tab2「单元格精修」
            CellRef 多区域（逗号分隔）+ 每区域独立策略（优先级 > 列配置）
        Tab3「预览并执行」
            50 行原值/掩码对比表，高亮被处理单元格
            执行前最后强提示：未勾选加密留存则无法直接还原
            点击执行 → 写入 3 个产物：
                1. {stem}_脱敏.xlsx    （样式保真）
                2. {stem}_脱敏.ecmap   （映射 + 策略快照 + header sha256 校验）
                3. {stem}.encrypted_src（可选，加密留存原件）
                4. {stem}_脱敏_report.md（可选 S-05 差异报告）
    ↓ 输出产物 → SandboxManager 列表 + 可选上传 FileBay
```

## 三、验收标准（AC，可量化）

> 使用 Given/When/Then 格式，P0/Must 100% 通过才准入 Step4 代码。

### 3.1 功能验收

| ID | 优先级 | Scenario（Given / When / Then） |
|---|---|---|
| AC-01 | P0/Must | Given：用户上传 `.xlsx`，settings 自动弹开关为开。When：上传完成。Then：1.5 秒内弹出脱敏对话框，对话框有关闭按钮（走旧流程）和「进入配置」主按钮；非 Excel/CSV 上传不弹。 |
| AC-02 | P0/Must | Given：关掉 settings 自动弹开关。When：上传 xlsx。Then：不弹对话框，走旧 preview_masking 流程；开关状态重启后保持。 |
| AC-03 | P0/Must | Given：10 万行 x 20 列 xlsx，首行为 header。When：解析结构。Then：10 秒内返回 headers + totalRows=100000 + totalCols=20 + sampleRows 前 50 行；首行全空时默认列 1..列 N 提示 banner。 |
| AC-04 | P0/Must | Given：Tab1 勾选手机号/身份证/姓名三列。When：预览并执行。Then：最终 xlsx 三列全部按策略掩码；随机抽 100 行三列值，掩码形状合规率 100%，未命中列不变。 |
| AC-05 | P0/Must | Given：Tab2 输入 CellRef `Sheet1!B2`、`Sheet1!A3:A20`、`Sheet1!C2:F100`，每个区域策略不同。When：执行。Then：抽样 20 个被覆盖单元格按区域策略掩码；非法 CellRef 输入立即红字报错禁止下一步。 |
| AC-06 | P0/Must | Given：姓名列用 FULL_MASK。When：「张三/张三丰/欧」。Then：掩码输出为 `**/****/*`（字长严格等于掩码 * 数量；中文 1 字 1 个 *）。 |
| AC-07 | P0/Must | Given：手机号列用 PHONE_MIDDLE_4，11 位手机号。When：执行。Then：保留前 3 + 后 4，中间 4 位为 `*`；空值或非 11 位走原值（不破坏）。 |
| AC-08 | P0/Must | Given：身份证列用 ID_MIDDLE_10，18 位身份证号。When：执行。Then：保留前 6 + 后 4，中间 8 位为 `*`（前 6 + 中 8 星 + 后 4 = 18，**v1.3 订正，见附录 B：v1.2 原文"中间 10 位"为算术错误**）；15 位老身份证保留前 3 + 后 4，中间 8 位掩码；空值/位数不符保留原值。 |
| AC-09 | P0/Must | Given：前端 + 后端策略枚举注册表；后端单元测试扫描字段类型占位。When：断言 BANK_CARD/EMAIL/ADDRESS/COMPLIANCE_ID 4 项存在启用占位或 disabled 占位。Then：4/4 全部存在；UI 对应 4 行「即将上线」灰态。 |
| AC-10 | P0/Must | Given：用户勾选加密留存（模式①复用沙箱口令）。When：执行成功后，同目录出现 `.ecmap` + `.encrypted_src`。Then：`.ecmap.header.sourceRetained=true` + `.ecmap.header.sourceEncryptionKeySource='SANDBOX_PASSPHRASE_REUSED'`；使用同沙箱口令 + CMAP_V1 域去解密 `.encrypted_src` 必然失败（SA-1 域分离）。 |
| AC-11 | P0/Must | Given：A 路径还原。When：用户选 `_脱敏.xlsx` + `.ecmap` + `.encrypted_src`（三文件同源）**+ 该 `.ecmap` 对应的解密口令/密钥来源（v1.3 补充，见附录 B）**。Then：还原后单元格与原输入值 1:1 相等；样式与原文件视觉一致率 ≥ 99%。 |
| AC-12 | P0/Must | Given：B 路径还原。When：用户选 `_脱敏.xlsx` + `.ecmap` + 用户原件（原件 sha256 == ecmap.header.originalSha256）**+ 该 `.ecmap` 对应的解密口令/密钥来源（v1.3 补充，见附录 B：B 路径是用用户原件替代 `.encrypted_src`，不是替代 `.ecmap` 解密口令）**。Then：还原后单元格与原件相等 1:1。 |
| AC-13 | P0/Must | Given：材料不足（只有 xlsx+ecmap，无 encsrc 也无原件）。When：点击还原。Then：红色 banner 强提示三选项；绝不进行「猜测还原」。 |
| AC-14 | P0/Must | Given：Tab0 三模式 radio，沙箱 store.passphrase 非空。When：页面初次挂载。Then：模式①被高亮选中，模式 ②/③ 可切；切到 ①后单独口令字段会被立即清空。 |
| AC-15 | P0/Must | Given：三提示区域：Tab0 顶部、执行前最后一步、反脱敏材料不足页。When：阅读文案 3/3 全部出现语句「不勾选加密留存将无法仅凭 .ecmap 还原」。Then：全部命中。 |
| AC-16 | P0/Must | Given：旧 IMP001 期间生成的 `.cmap`（现有 fixture 3 个）。When：调用 `load_encrypted_mapping` 新 helper。Then：解密成功，不触发 SA-5 破坏。 |
| AC-17 | P1/Should | Given：企业版上传同名结构 Excel，导入模板 JSON + 按列名精确匹配。When：应用模板。Then：≥ 90% 列自动打勾，10% 未匹配提示手动覆盖。**（v1.3 补充：P1，本期不实施，不构成本轮发布阻断，见附录 B）** |
| AC-18 | P1/Should | Given：10 万行 × 20 列，开启加密留存（模式①）。When：端到端执行（上传→弹框→配置→执行）。Then：总时长 ≤ 120 秒；50 行预览 ≤ 2 秒；有进度条每 500ms 更新。**（v1.3 补充：P1，本期不实施，不构成本轮发布阻断，见附录 B）** |
| AC-19 | P1/Should | Given：图表饼图引用 C 列数据，C 列被策略加密（掩码值）。When：打开脱敏 xlsx。Then：图表仍正常渲染，显示值为掩码后的新值，不出现空白图。**（v1.3 补充：P1，本期不实施，不构成本轮发布阻断，见附录 B）** |
| AC-20 | P1/Should | Given：用户忘记当时沙箱口令。When：反脱敏自动填失败。Then：passphraseDomainHint8 快速判断不匹配；立即提示切换旧口令或 B 路径，不浪费 3 秒 PBKDF2 算力误判。**（v1.3 补充：P1，本期不实施，不构成本轮发布阻断，见附录 B）** |

### 3.2 性能验收

1. 解析速度：10 万行 × 20 列 Excel 读取结构 ≤ 10 秒。
2. 执行速度：10 万行 × 20 列 → 产出 3 文件端到端 ≤ 120 秒（含 AES-GCM 加密 + OOXML 样式写回）。
3. 预览速度：50 行对比预览 ≤ 2 秒。
4. 内存峰值（Tauri 主进程）：中等规模文件 ≤ 1.5GB；大文件启用流式写回后仍 ≤ 2GB。
5. 反脱敏速度：同规模文件 A/B 路径 ≤ 60 秒。

### 3.3 安全与合规验收

1. SAST（cargo-audit / pnpm audit）0 Critical。
2. SCA 0 High。
3. PII 字段存储：`.encrypted_src` 一律 AES-256-GCM（文件级随机盐 + PBKDF2-HMAC-SHA256 20 万次迭代 + 域分离前缀）。
4. 日志零明文：所有 processing_history、埋点、控制台只允许 PII 哈希前缀与友好掩码前缀。
5. 口令零持久化：单独口令 SECONARY_PASSPHRASE 绝不写入 persist store；Rust 侧传参一律 zeroize。
6. 域分离：T-PR-02 用例自动化通过（同口令跨 CMAP/ENCSRC 域互解必然失败）。
7. 向后兼容：T-PR-06 自动化通过。

### 3.4 兼容性验收

1. 操作系统：macOS 13+（Apple Silicon + Intel）、Windows 10+ x64、Ubuntu 22.04+ x64。
2. 浏览器：Tauri WebView2（Win）/ WKWebView（macOS）/ WebKitGTK 2.40+（Linux）；分辨率 ≥ 1280×800。
3. 文件格式兼容：.xlsx（Excel 2007+）、.xls（Excel 97-2003）、.csv（UTF-8/GBK/GB18030 自动识别）。**`.xlsm` 不在此列，见 §1.3 非目标 6（v1.3 新增）：不承诺宏保留，不作为 P0 阻断项。**

## 四、优先级（MoSCoW）

| 子功能 | 优先级 | 对应 ID |
|---|---|---|
| F01 上传 Excel/CSV 自动弹配置对话框（含 settings 总开关） | Must | M-13/AC-01/AC-02 |
| F02 工作表结构解析（列名/行数列数/多 sheet 切换 S-03） | Must | M-02/AC-03 |
| F03 字段级整列批量脱敏（自动识别 PII 列 + 勾选） | Must | M-03/S-02/AC-04 |
| F04 单元格级精细化脱敏（任意区域 CellRef 覆盖列策略） | Must | M-04/AC-05 |
| F05 脱敏策略矩阵：姓名/手机/身份证/4 预留 | Must | M-05/06/07/08/AC-06-09 |
| F06 映射文件 .ecmap 生成（含 header 元数据 + sha 校验） | Must | M-09/AC-10 |
| F07 反脱敏还原（A/B 两路径 + 快速提示） | Must | M-10/AC-11-13/AC-20 |
| F08 加密源文件留存三模式（默认复用沙箱口令） | Must | M-15/M-17/AC-10/AC-14 |
| F09 合规提示文案三区域（说明不勾选的后果） | Must | M-16/AC-15 |
| F10 50 行 live 预览 + 高亮被处理单元格 | Must | M-11 |
| F11 样式保真（公式/图表/条件格式/批注/合并单元格保留） | Must | M-14/AC-19 |
| F12 个人版/企业版同能力上线（无差异禁用） | Must | M-12 |
| F13 企业版策略模板导入/导出（JSON） | Should | S-01/AC-17 |
| F14 智能列类型识别（关键字+抽样正则） | Should | S-02 |
| F15 多 Sheet 下拉切换 | Should | S-03 |
| F16 进度条 + 可取消（10 万行进度提示） | Should | S-04/AC-18 |
| F17 差异高亮 report.md（统计策略命中数） | Should | S-05 |
| F18 用户自定义正则临时策略（本次不入库） | Could | C-01 |
| F19 .ecmap 密码加密（AES-GCM passphrase 保护 ecmap 本身） | Could | C-02（SA-1 中已区分域，当前默认 .ecmap 与 .encrypted_src 各走各自域加密） |
| F20 多文件批量上传同一模板 | Could | C-03 |
| F21 非 Excel/CSV 结构化格式（Parquet/ODS/XLSB） | Won't | W-01 |
| F22 VBA 宏执行/签名保证（仅保留宏流不主动运行） | Won't | W-02 |
| F23 样式 100% 字节级一致（仅视觉一致即可，允许 OOXML 重排） | Won't | W-03 细节 |
| F24 云端多人协同编辑脱敏 | Won't | W-04 |

## 五、依赖关系 & 上下游影响

### 5.1 内部依赖

1. CheersAI-Vault 前端 zustand store：[fileStore.ts](../../src/store/fileStore.ts)（沙箱口令 + outputDir）。
2. Rust 后端命令层：[masking.rs](../../src-tauri/src/commands/masking.rs) + [crypto.rs](../../src-tauri/src/core/crypto.rs)。
3. 共享解析层 calamine + engine-core parser（InputFormat::Excel）。
4. UI 组件库：shadcn/ui Dialog / RadioGroup / Tabs / Switch + 现有 [PassphraseBox.tsx](../../src/components/common/PassphraseBox.tsx)。
5. Sandbox 产物列表：[SandboxManagerDesktop.tsx](../../src/pages/SandboxManagerDesktop.tsx)；FileManager 要新增 `.encrypted_src`、`.ecmap` 两类文件的展示与批量删除。

### 5.2 外部依赖

1. Rust crate：`rust_xlsxwriter`（写回样式保真）、`calamine`（读）、`hkdf`（DEVICE_KEY HKDF）、`keyring`（OS 级 keychain）、`zeroize`（口令清除）。
2. 现有 crate `aes-gcm`、`sha2`、`pbkdf2`（已存在，升级迭代次数常量即可）。
3. 外部密钥服务：暂无；企业版未来 KMS 接入预留即可。

### 5.3 风险 & 缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|---|---|---|---|
| 样式保真方案在部分复杂 Excel（SmartArt、嵌入 OLE 对象）有遗漏 | 中 | 中 | 降级路径：excel-style-core OOXML 直接克隆 + 值替换 + report.md 中写「降级声明」列出受影响 sheet 与对象类型；用户允许手动退回旧 Markdown 路径（可选项） |
| 10 万行文件 OOM | 中 | 高 | 流式 + 分页写回；内存峰值 ≥ 2GB 时弹窗提示关闭其他程序；限制 30 万行硬上限（超过提示「请拆分」，不在本 PRD 覆盖大规模能力） |
| 用户把沙箱口令改了后，旧 .encrypted_src 无法直接还原 | 中 | 中 | SA-4 passphraseDomainHint8 提前失败 + 提示：请输入「加密当时」的沙箱口令；也可走 B 路径持原件还原 |
| DEVICE_KEY 在 macOS Keychain 被用户手动删除 | 低 | 高 | 删除前首次生成时在设置页弹窗「建议：导出 DEVICE_KEY 的恢复码（仅一次）」；企业版可通过 Nexus 下发恢复码托管 |
| 沙箱口令复用 → 域分离实现有 bug → 可跨域互解 | 极低 | 极高 | T-PR-02 必须自动化；PR 合入前 CI 必跑；加 fuzz 测试 10k 次不出现跨域成功 |

## 六、合规与数据分级（逐字段标注）

> 所有涉及的字段；C3+ 必须标注加密存储/脱敏展示/日志零明文。

| 字段名 | 分类 | 加密存储 | 脱敏展示（策略矩阵） | 日志允许明文 |
|---|---|---|---|---|
| 用户真实姓名（列） | C3 | Y（.encrypted_src） | NAME_FULL_MASK / CLEAR | N（仅允许「张*」+ sha256[:12]） |
| 手机号（列） | C4 | Y | PHONE_MIDDLE_4 / DEFAULT / CLEAR | N（仅允许「139****」+ sha256[:12]） |
| 身份证号（列） | C4 | Y | ID_MIDDLE_10 / DEFAULT / CLEAR | N（仅允许「110101********1234」友好掩码 + sha256[:12]） |
| 邮箱地址（预留 BANK_CARD/EMAIL/ADDRESS/COMPLIANCE_ID 四类） | C3 | Y | UI 占位「即将上线」，不产生实际脱敏 | N |
| 银行卡号（预留） | C4 | Y | 占位 | N |
| 家庭住址（预留） | C3 | Y | 占位 | N |
| 沙箱口令（用户输入/设置） | C3 | N（仅内存短活；persist 只在 rememberPassphrase=true 且由用户显式启用） | 前端绝不展示明文（默认星号），允许显示长度 | N |
| 单独设置的本次加密口令 | C3 | N（内存短活；全程 zeroize） | 绝不展示 | N |
| DEVICE_KEY（本机系统 keyring） | C4 | Y（仅 OS keychain，Vault 进程只拿派生后 HKDF session key） | 绝不展示 | N（仅允许展示设备 key 存在性布尔值） |
| .ecmap header.originalSha256 | C2 完整性 | N（仅完整性） | 展示全 SHA256（非明文） | Y |
| .ecmap entries.originalSha256 | C2 完整性 | N（仅完整性） | 展示前 12 位 | Y（前 12 位可日志） |
| .ecmap entries.originalPreview（友好前缀，如 张*） | C2 | N | 仅预览使用；绝不落入 audit 明文字段 | N（审计日志只接受 sha，友好前缀仅 UI 用） |
| 沙箱 outputDir 路径 | C1 | N | 用户设置页可查看完整路径 | Y（但路径中若含 PII，用户自行规避） |

## 七、验收通过证据清单（G4 输入）

1. [ ] 功能测试报告（P0 AC-01 ~ AC-16 100% 通过；P1 ≥ 95%）。
2. [ ] 性能压测报告（10 万行 × 20 列，见 §3.2 五条全达标）。
3. [ ] 安全扫描：cargo-audit + cargo-deny + pnpm audit + npm SBOM（SAST 0 Critical，SCA 0 High）。
4. [ ] 兼容性矩阵报告：macOS / Win / Ubuntu 三套系统 + Apple Silicon x86 双架构，全绿。
5. [ ] 域分离安全用例 T-PR-02 报告。
6. [ ] 旧版本 .cmap 兼容用例 T-PR-06 报告。
7. [ ] 文案检查：Tab0 / 执行前 / 反脱敏页三区域三提示文案截图确认。
8. [ ] 代码 review 双签字（TD+RO，HE-5 工装变更不涉及本 PRD）。

## 附录 A：变更历史

| 版本 | 日期 | 修改人（RA） | 修改说明 |
|---|---|---|---|
| v1.0 | 2026-08-15 | sdlc-ra-002@cheersai.ai | 初稿（v1.0 基线：5 项用户确认） |
| v1.1 | 2026-08-15 | sdlc-ra-002@cheersai.ai | 升级 M-13/M-14/M-15/M-16，取消 W-03，升级为 M |
| v1.2 | 2026-08-15 | sdlc-ra-002@cheersai.ai | 新增 M-17（沙箱口令复用 + 三模式 + SA-1~SA-7 安全保证），基线冻结 6 条 |
| v1.3 | 2026-08-27 | CheersAI Vault 项目组 | 基于 v1.2 的最小增量更新：身份证掩码算式、P0 格式边界、A/B 恢复口令语义及 P1 延后边界；详见附录 B。 |

## 附录 B：v1.2 → v1.3 变更清单（可审计 diff，逐项映射任务授权）

本附录列出 v1.3 相对 v1.2 全文的全部实质差异；未列出的正文、表格、章节标题和编号均保持 v1.2 原意。

| # | 位置 | v1.2 原文 | v1.3 改动 | 映射授权类别 |
|---|---|---|---|---|
| 1 | 文档头部 | `版本: v1.2` | 版本更新为 `v1.3（基于 v1.2 最小增量）`，并声明为当前 Excel P0 实施与验收基线 | 文档版本管理 |
| 2 | §1.3 非目标 | 仅 5 条非目标 | 新增"非目标 6"，说明 `.xlsm` 不在 P0 输入格式范围、不作为 P0 阻断项 | 格式边界 |
| 3 | AC-08（§3.1） | "保留前 6 + 后 4，中间 **10** 位为 `*`" | 改为"保留前 6 + 后 4，中间 **8** 位为 `*`（前 6 + 中 8 星 + 后 4 = 18）"，并加注"v1.3 订正，v1.2 原文『中间 10 位』为算术错误" | AC-08 算式（负责人确认） |
| 4 | AC-11（§3.1） | Given 只列三文件同源 | Given 追加"+ 该 `.ecmap` 对应的解密口令/密钥来源" | 路径 A/B 材料及口令语义 |
| 5 | AC-12（§3.1） | Given 只列脱敏文件+ecmap+用户原件 | Given 追加"+ 该 `.ecmap` 对应的解密口令/密钥来源"，并加括注说明"B 路径是用用户原件替代 `.encrypted_src`，不是替代 `.ecmap` 解密口令" | 路径 A/B 材料及口令语义 |
| 6 | AC-17～AC-20（§3.1） | 无 P1 边界显式标注（仅优先级列已写 P1/Should） | 每行 Then 后追加"（P1，本期不实施，不构成本轮发布阻断）" | P1 延后边界 |
| 7 | §3.4 兼容性验收第 3 条 | 只列 `.xlsx`/`.xls`/`.csv` | 追加一句"`.xlsm` 不在此列……不承诺宏保留，不作为 P0 阻断项" | 格式边界 |
