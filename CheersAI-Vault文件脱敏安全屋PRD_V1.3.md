

## 文档信息

| 属性 | 内容 |
|------|------|
| 文档版本 | V1.3 |
| 创建日期 | 2026-03-21 |
| 产品负责人 | 西北人 |
| 技术负责人 | 王再旺 |
| 设计负责人 | 西北人 |
| 文档状态 | 初稿 |

---

## 目录

1. [项目概述](#1-项目概述)
2. [产品定位](#2-产品定位)
3. [目标用户](#3-目标用户)
4. [用户故事](#4-用户故事)
5. [系统架构](#5-系统架构)
6. [核心能力设计](#6-核心能力设计)
7. [API/SDK 设计](#7-apisdk-设计)
8. [数据模型](#8-数据模型)
9. [安全设计](#9-安全设计)
10. [验收标准](#10-验收标准)
11. [MoSCoW 优先级](#11-moscow-优先级)
12. [实施计划](#12-实施计划)
13. [风险与应对](#13-风险与应对)
14. [附录](#14-附录)

---

## 1. 项目概述

### 1.1 背景

在 CheersAI 产品矩阵中，AI 辅助办公场景涉及大量敏感文档处理，包括客户信息、合同条款、财务数据、商业机密等。这些敏感数据在进入 AI 处理流程前必须经过脱敏处理，以满足金融、医疗、法律等行业的合规要求。

当前面临的三个核心困境：

**安全困境**：传统 AI 文档处理需要将文件上传云端，这对银行、医疗机构、政府单位而言是合规红线。一旦敏感数据离开本地环境，即面临数据泄露、监管处罚、客户信任丧失等风险。Vault 正是为解决这一困境而设计的安全基础设施。

**配置困境**：现有的脱敏方案往往需要用户手动配置规则，对非技术人员不友好。Vault 通过预置行业模板和自动实体识别，降低配置门槛，实现开箱即用。

**运营困境**：脱敏后的数据如何追踪、审计、还原，是运维团队头疼的问题。Vault 提供完整的审计日志和可控还原机制，让运维人员有据可依、有迹可查。

### 1.2 产品目标

**V1.0 核心目标**：建立本地化的文件脱敏处理引擎，实现敏感数据的本地识别、标记、脱敏全流程，确保数据不出设备、不上传云端，为 CheersAI Desktop 及相关产品提供安全可靠的数据处理能力。

**V1.0 质量目标**：

- 脱敏召回率 (Recall) 不低于 99%，即 100 个敏感实体中至少识别出 99 个
- 脱敏精确率 (Precision) 不低于 95%，即识别出的敏感实体中至少 95% 是真正的敏感信息
- 文件处理完成后，原文件从内存和临时存储中完全清除
- 所有操作生成不可篡改的审计日志

**V1.1 增强目标**：在 V1.0 基础上增加策略审批流、反脱敏回填、DLP 风险提示，并增强自动化、批量处理、可扩展性与开放 API 能力。

### 1.3 产品范围

**包含范围**：

- 多格式文件的本地解析与敏感实体识别
- 基于 SLM 引擎和规则引擎的双引擎实体识别
- Span-level 精确脱敏与 FPE 可逆还原
- 本地 AES-256 加密沙箱
- 完整的审计日志体系
- 行业预置脱敏模板
- 对外暴露的 IPC API 接口，供 Desktop/Desktop 服务端（Dify）/FileBay 等产品调用

**不包含范围**：

- 独立的桌面应用程序（Vault 是后台服务组件）
- 云端处理能力（完全本地化设计）
- AI 模型推理能力（仅负责脱敏，AI 推理由调用方处理）
- 跨设备的文件同步
- 实时网络威胁防护

### 1.4 Non-Goals

以下功能在 V1.0 明确不包含，需在后续版本中评估：

- 脱敏策略审批流（V1.1 企业版）
- 反脱敏自动回填功能（V1.1 企业版）
- DLP 风险提示（V1.1 企业版）
- 云端策略管理控制台
- 多租户隔离架构
- 离线增量模型更新
- 视频/音频文件的脱敏处理

---

## 2. 产品定位

### 2.1 战略定位

CheersAI Vault 是 CheersAI 产品生态的安全核心基础设施，定位为"文件脱敏安全屋"——一个独立的、可复用的本地文件安全处理引擎。

**一句话定位**：CheersAI 产品生态的安全核心——敏感数据本地处理、不出设备、不上传云端。

**核心理念**：

- 脱敏在本地：所有敏感数据识别和脱敏操作在本地完成
- 数据不离端：文件内容在 Vault 内部处理，不经过任何网络传输
- 反脱敏可控：仅对授权输出通道开放可逆还原能力
- 策略可配置：支持灵活的脱敏规则和行业模板

### 2.2 产品角色

Vault 在 CheersAI 产品矩阵中承担安全基础设施的角色：

```
┌─────────────────────────────────────────────────────────────┐
│                    CheersAI 产品矩阵                          │
├─────────────────────────────────────────────────────────────┤
│  CheersAI Desktop  — 桌面客户端                               │
│  Desktop 服务端（Dify）   — 云端AI能力平台（基于Dify，承载模型/智能体/工作流/知识库/Prompt） │
│  CheersAI Nexus    — 运营管理中枢                             │
│  CheersAI SSO      — 统一身份认证                             │
│  CheersAI Vault    — 文件脱敏安全屋（本文档）                 │
│  CheersAI FileBay  — 脱敏文件协作平台（云端脱敏文件存储/版本/共享/审批） │
│                                                             │
│  关系：Vault 脱敏 -> FileBay 云端管理                         │
└─────────────────────────────────────────────────────────────┘
```

**与 Desktop 的关系**：

- Desktop 是面向用户的办公客户端，承担 UI 交互和用户引导
- Vault 是面向安全的文件处理引擎，承担所有敏感数据处理逻辑
	- *Desktop 的所有脱敏功能调用 Vault API 实现* 【有出入】
	- *用户感知不到 Vault 的存在，Vault 作为后台服务静默运行【有出入】*

**与 Desktop 服务端（Dify） 的关系**：

- Desktop 服务端（Dify） 提供云端 AI 能力，企业可通过本地部署的 Vault 先完成脱敏
- 脱敏后的数据进入 Desktop 服务端（Dify） 的模型/工作流处理，原始数据不出设备

**与 FileBay 的关系**：

- Vault 输出脱敏文件后，可一键上传到 FileBay 进行云端存储与协作
- FileBay 不保存映射表，映射表留在 Vault 的加密沙箱内

**可扩展性**：

- Nexus 可接入 Vault 的审计/统计数据，用于运营与合规管理
- SSO 为 Vault 提供统一身份认证与授权能力
- 未来可扩展接入更多 CheersAI 产品与企业系统

### 2.3 技术定位

Vault 技术层面的定位是**轻量级安全中间件**：

- 运行时：作为独立进程运行，通过 IPC 与调用方通信
- 部署：与应用程序打包部署，不依赖独立的服务器
- 资源占用：内存占用控制在 200MB 以内（不含模型权重）
- 启动时间：冷启动不超过 3 秒，热启动不超过 500ms

### 2.4 竞品分析

| 产品 | 厂商 | 核心能力 | CheersAI Vault 对标 |
|------|------|---------|-------------------|
| Data Masking | Informatica | 企业级静态/动态脱敏 | 静态脱敏 + FPE |
| Data Redaction | Oracle | 数据库层脱敏 | 文件层脱敏（差异化） |
| PrivacyGuard | IBM | 合规驱动脱敏 | 合规模板 + 审计 |
| Protegrity | Protegrity | 令牌化保护 | FPE + 映射表 |
| DataVisor | 国产 | AI 驱动脱敏 | 双引擎(SLM+规则) |

Vault 差异化优势：

- 本地化处理（竞品多为云端/数据库层）
- AI 驱动识别（双引擎，非纯规则）
- 面向办公文档（竞品面向数据库/数据仓库）
- 与 AI 办公流程深度集成

---

## 3. 目标用户

### 3.1 用户画像

Vault 涉及三类用户群体，每类用户对 Vault 的使用方式和关注点不同：

**第一类：间接用户（通过 Desktop 使用）**

| 属性 | 描述 |
|------|------|
| 用户规模 | CheersAI Desktop 的所有注册用户 |
| 典型场景 | 银行客户经理使用 AI 辅助撰写尽调报告 |
| 关注点 | 脱敏过程无感知、操作简便、处理速度快 |
| 使用方式 | 在 Desktop 中上传文件，Vault 在后台自动处理 |
| 痛点 | 不想知道脱敏是什么，只想文件能被 AI 正常处理 |

**第二类：安全/合规负责人**

| 属性 | 描述 |
|------|------|
| 用户规模 | 企业的信息安全部门、合规部门 |
| 典型场景 | 银保监局检查时需要提供数据处理审计记录 |
| 关注点 | 合规认证、审计日志完整性、策略有效性 |
| 使用方式 | 配置脱敏策略、导出审计日志、查看处理报表 |
| 痛点 | 需要向监管机构证明数据处理符合规范 |

**第三类：内部开发团队**

| 属性 | 描述 |
|------|------|
| 用户规模 | CheersAI 内部 SDK 使用者 |
| 典型场景 | 将 Vault 集成到新的产品线中 |
| 关注点 | API 完备性、文档清晰度、集成便捷性 |
| 使用方式 | 集成 SDK 到目标产品，调用 Vault API |
| 痛点 | 需要快速接入、避免踩坑 |

### 3.2 用户需求优先级

| 用户类型 | 需求优先级 |
|----------|-----------|
| 间接用户 | 1. 数据安全（不泄露） 2. 功能可用（能处理） 3. 性能体验（处理快） |
| 安全负责人 | 1. 合规可证（能审计） 2. 策略可控（能配置） 3. 日志完整（可追溯） |
| 开发团队 | 1. API 完备（能调通） 2. 文档清晰（能看懂） 3. 集成便捷（能上线） |

---

## 4. 用户故事

### 4.1 用户故事编写规范

本PRD采用 Job Story 格式编写用户故事，每个故事包含：

- **背景 (Context)**：用户所处的情境
- **动机 (Motivation)**：用户想要达成什么
- **期望结果 (Outcome)**：用户期望得到什么

验收标准采用 Given-When-Then (GWT) 格式编写。

### 4.2 核心用户故事

---

**JS-001：银行客户经理处理尽调报告**

**背景**：某银行客户经理需要使用 Desktop 的 AI 辅助功能撰写客户尽调报告。她需要上传客户提交的历史合同、财务报表等文件，但这些文件包含大量客户隐私信息（身份证号、银行账号、企业信用代码等）。

**动机**：她希望 AI 能够理解和分析这些文档的内容和结构，但不希望原始敏感数据被上传到任何云端服务器，也不希望在处理过程中被任何人看到。

**期望结果**：上传文件后，AI 能够准确理解文档的业务含义并给出有价值的分析建议，而文件中所有可识别的敏感信息都被自动替换或遮蔽。

**验收标准 (GWT)**：

```
Given: 用户上传一份包含客户身份证号(310101199001011234)
      和银行账号(6222021234567890123)的 PDF 合同文件

When: Desktop 调用 Vault 进行脱敏处理

Then: 脱敏后的文件内容中，身份证号和银行账号被替换为脱敏标记
      原文件中身份证号和银行账号的字符数保留(如: 31****19900101****)
      原始文件从处理节点的内存和临时存储中完全清除
      审计日志记录本次脱敏操作，包含操作时间戳和文件哈希
```

---

**JS-002：合规负责人配置金融行业脱敏策略**

**背景**：某城商行合规负责人收到监管要求，需要确保所有 AI 处理的客户数据必须经过脱敏处理。他需要为业务部门配置统一的脱敏规则，覆盖客户姓名、身份证号、银行卡号、手机号、地址等信息。

**动机**：他希望有一套预置的金融行业模板，可以一键应用，同时能够根据本行的特殊要求进行微调。

**期望结果**：能够选择"金融行业-银行标准"模板，确认模板内容符合要求后，一键发布到全行使用。

**验收标准 (GWT)**：

```
Given: 合规负责人首次登录 Desktop 的策略配置界面

When: 他选择"金融行业-银行标准"模板并点击"应用"

Then: 系统展示模板包含的所有实体类型和对应的脱敏规则
      规则包括：姓名->替换为"客户A/B/C"、身份证号->保留前6后4
      手机号->138****xxxx、银行卡号->只保留卡种类型标识
      确认发布后，规则立即生效，所有新上传文件自动应用该规则
```

---

**JS-003：审计人员导出合规审计报告**

**背景**：某证券公司迎接证监会现场检查，审计人员需要提供过去三个月内所有 AI 文档处理操作的记录，证明客户数据在处理过程中得到妥善保护。

**动机**：他需要一份完整的、可追溯的审计日志，包含每个文件的处理时间、操作类型、使用的脱敏规则等信息，且日志本身需要防篡改。

**期望结果**：能够按时间范围、文件类型、操作类型等条件筛选审计日志，一键导出为标准格式的报告文件。

**验收标准 (GWT)**：

```
Given: 审计人员需要导出 2025-10-01 至 2025-12-31 的审计报告

When: 他在 Desktop 中选择"导出审计报告"，设置时间范围后点击导出

Then: 系统生成包含以下字段的 CSV 报告：
      - 记录ID（不可重复）
      - 操作时间（精确到毫秒）
      - 操作类型（脱敏/还原/外发）
      - 文件名
      - 文件哈希(SHA-256)
      - 使用的规则名称
      - 操作用户
      - 操作结果（成功/失败）
      - 日志签名（用于校验完整性）
      报告文件同样存储于加密沙箱中
```

---

**JS-004：开发团队集成 Vault SDK**

**背景**：CheersAI 开发团队需要在 Desktop 服务端（Dify） 中集成文件脱敏功能。Desktop 服务端（Dify） 是云端 AI 能力平台，需要在企业侧文件进入云端工作流前完成本地脱敏。

**动机**：他们希望有一套完善的 SDK，文档清晰、接口规范、错误处理完善，能够在一天内完成集成工作。

**期望结果**：通过 SDK 的简单几行代码调用，即可完成文件脱敏的完整流程，无需了解底层实现。

**验收标准 (GWT)**：

```
Given: 开发团队需要为 Desktop 服务端（Dify） 集成文件脱敏功能

When: 开发团队阅读 SDK 文档后，编写以下代码：
      const vault = new VaultClient();
      const result = await vault.maskFile(filePath, {
        template: 'financial-bank',
        mode: 'strict'
      });

Then: SDK 返回脱敏后的文件路径和识别出的实体摘要
      实体摘要包含：实体类型、数量、脱敏位置
      若文件格式不支持，返回明确的错误码和提示
      SDK 自动处理文件加密沙箱的读写权限验证
```

---

**JS-005：用户处理需要还原的合同条款**

**背景**：某律师使用 AI 分析一份合同文本，分析完成后发现部分条款需要确认原始内容。该律师拥有文件的合法访问权限，需要查看被脱敏部分的原始内容。

**动机**：他希望能够在受控环境下还原脱敏内容，但不希望所有人都能还原，且还原操作需要有完整的记录。

**期望结果**：能够对自己有权限的文件进行受控还原，还原操作被完整记录，可以随时查看谁在什么时间还原了什么内容。

**验收标准 (GWT)**：

```
Given: 用户已完成合同文件的脱敏处理，现在需要查看原文中某处被脱敏的人名

When: 用户在 Desktop 中选择"查看原始内容"，系统验证用户权限后显示还原结果

Then: 仅被还原的实体内容展示给用户（该人名）
      还原操作记录到审计日志，包含：操作人、被还原的实体类型、
      对应的映射表ID、时间戳
      高风险实体（如身份证号、银行账号）的还原需要二次验证
```

---

**JS-006：医疗从业者处理病历文档**

**背景**：某医院信息科需要配置 AI 辅助诊断系统，该系统会处理包含患者隐私的病历文档。需要确保患者姓名、身份证号、诊断结果等敏感信息在进入 AI 分析前被脱敏。

**动机**：他们希望有一套医疗行业的脱敏模板，覆盖病历中常见的敏感字段，同时保留诊断分析所需的医学术语和检查数据。

**期望结果**：能够选择"医疗行业-病历标准"模板，自动识别并脱敏患者基本信息，同时保留检查项目、检验数值等可分析数据。

**验收标准 (GWT)**：

```
Given: 用户上传一份包含患者姓名、身份证号、诊断报告的病历文档

When: 系统应用"医疗行业-病历标准"模板进行脱敏

Then: 患者姓名->脱敏标记（如"患者001"）
      身份证号->保留出生日期部分（如19xxxxxx）
      诊断报告中的诊断结果、检验数值保持不变
      以便 AI 能够分析病情但无法识别具体患者身份
```

---

**JS-007：处理代码文件中的敏感信息**

**背景**：某开发者在代码审查场景中使用 AI 辅助分析代码安全性。代码文件中可能包含 API Key、私有函数名、内部类名、数据库连接字符串等敏感信息。

**动机**：他希望 AI 能够理解代码的业务逻辑和安全架构，但不希望将内部实现细节（如私有 API、API Key）暴露给 AI 或其他方。

**期望结果**：上传代码文件后，AI 能够分析代码逻辑，但代码中的私有标识符被脱敏处理。

**验收标准 (GWT)**：

```
Given: 用户上传一个包含数据库连接串和 API Key 的 Python 代码文件
      db_password = "prod_db_2024_secret"
      api_key = "sk-abc123def456xyz789"

When: Vault 对代码文件进行脱敏处理

Then: 数据库连接串中的密码部分被脱敏（如"prod_db_****"）
      API Key 被替换为通用占位符（如"[REDACTED_API_KEY]"）
      私有函数名(如__internal_process)被替换为func_001
      公共函数名和分析所需的业务逻辑保持可读性
```

---

**JS-008：处理 PPT 中的文本框和图形文字**

**背景**：某咨询顾问需要使用 AI 分析一份包含客户案例的 PPT 文件。PPT 中不仅有普通文本框，还包含形状内的文字、图表标注等。

**动机**：他希望 PPT 中的所有文字内容都能被 AI 识别和理解，包括嵌入图形中的文字，但所有敏感信息都需要被脱敏。

**期望结果**：PPT 中的所有文字被完整提取和脱敏，无论是普通文本还是图形内嵌文字。

**验收标准 (GWT)**：

```
Given: 用户上传一个包含多种元素的 PPT 文件：
      - 普通文本框中的客户名称
      - 圆形形状中的联系方式
      - 图表坐标轴标注中的金额数据

When: Vault 解析 PPT 并进行脱敏处理

Then: 所有文本框内容被提取和脱敏
      图形（如圆形）内的文字同样被提取和脱敏
      图表标注中的数值（如"1000万")被识别为金额并脱敏
      脱敏后的 PPT 保持原有格式和布局
```

---

### 4.3 用户故事汇总

| ID | 用户类型 | 故事主题 | 优先级 |
|----|----------|----------|--------|
| JS-001 | 间接用户 | 处理尽调报告 | P0 |
| JS-002 | 安全负责人 | 配置金融脱敏策略 | P0 |
| JS-003 | 安全负责人 | 导出合规审计报告 | P0 |
| JS-004 | 开发团队 | 集成 Vault SDK | P0 |
| JS-005 | 间接用户 | 还原脱敏内容 | P1 |
| JS-006 | 间接用户 | 处理病历文档 | P1 |
| JS-007 | 间接用户 | 处理代码文件 | P1 |
| JS-008 | 间接用户 | 处理 PPT 图形文字 | P2 |

---

## 5. 系统架构

### 5.1 整体架构

Vault 采用分层架构设计，从下到上分为四层：

```
┌─────────────────────────────────────────────────────────────┐
│                      调用方层 (Caller Layer)                  │
│   CheersAI Desktop | Desktop 服务端（Dify） | CheersAI FileBay | ... │
└─────────────────────────────┬───────────────────────────────┘
                              │ IPC (Inter-Process Communication)
┌─────────────────────────────▼───────────────────────────────┐
│                      接口层 (Interface Layer)                 │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐          │
│  │vault:   │ │vault:   │ │vault:   │ │vault:   │  ...      │
│  │mask     │ │restore  │ │audit    │ │config   │          │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘          │
└─────────────────────────────┬───────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────┐
│                      核心引擎层 (Core Engine Layer)           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │ 文件解析引擎   │  │ 实体识别引擎   │  │ 脱敏执行引擎   │         │
│  │File Parser   │  │NER Engine    │  │Mask Engine   │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │ FPE加密引擎   │  │ 审计日志引擎   │  │ 策略配置引擎   │         │
│  │FPE Engine    │  │Audit Engine  │  │Policy Engine │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
└─────────────────────────────┬───────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────┐
│                      安全存储层 (Secure Storage Layer)        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │ 加密文件系统   │  │  映射表存储    │  │ 密钥管理      │        │
│  │Encrypted FS  │  │Mapping Store │  │Key Manager   │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 与 Desktop 的集成方式

Desktop 与 Vault 通过本地 IPC 通道通信，Vault 以后台服务形式运行：

```
┌─────────────────────────────────────────────────────────────┐
│                      CheersAI Desktop                         │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    Desktop UI Layer                    │   │
│  │   文件上传 | 脱敏配置 | 进度展示 | 结果预览 | 审计查看  │   │
│  └─────────────────────────────┬────────────────────────┘   │
│                                │                             │
│  ┌─────────────────────────────▼────────────────────────┐   │
│  │                 Desktop Service Layer                 │   │
│  │   文件管理 | 用户交互 | 调用 Vault API | 状态同步      │   │
│  └─────────────────────────────┬────────────────────────┘   │
│                                │ IPC (Domain Socket/命名管道) │
└────────────────────────────────┼──────────────────────────────┘
                                 │
                                 ▼
┌────────────────────────────────────────────────────────────────┐
│                        CheersAI Vault                           │
│                                                                │
│  ┌────────────────────────────────────────────────────────┐    │
│  │                    Vault Service                        │    │
│  │  监听 IPC 通道 | 解析请求 | 路由分发 | 权限校验         │    │
│  └────────────────────────────┬───────────────────────────┘    │
│                               │                                │
│  ┌────────────────────────────▼───────────────────────────┐    │
│  │                   核心处理引擎                          │    │
│  │  文件解析 -> 实体识别 -> 策略匹配 -> 脱敏执行 -> FPE     │    │
│  └────────────────────────────┬───────────────────────────┘    │
│                               │                                │
│  ┌────────────────────────────▼───────────────────────────┐    │
│  │                   加密存储层                             │    │
│  │  映射表 | 密钥 | 审计日志 | 处理缓存                     │    │
│  └─────────────────────────────────────────────────────────┘    │
└────────────────────────────────────────────────────────────────┘
```

### 5.3 与 FileBay 集成

- 脱敏完成后可一键上传到 FileBay
- FileBay 接收文件时验证脱敏状态（必须经过 Vault 处理）
- 映射表不上传 FileBay（分离存储原则）
- 从 FileBay 下载文件后可通过 Vault 还原

### 5.4 核心处理流程

```
用户上传文件
      │
      ▼
┌─────────────┐
│ 文件导入    │ ─── 写入加密沙箱
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ 格式解析    │ ─── 提取文本内容、布局信息
└──────┬──────┘
       │
       ▼
┌─────────────┐     ┌─────────────────┐
│ 实体识别    │ ──> │ SLM 引擎 + 规则引擎 │
└──────┬──────┘     └─────────────────┘
       │
       ▼
┌─────────────┐
│ 策略匹配    │ ─── 应用配置的脱敏规则
└──────┬──────┘
       │
       ▼
┌─────────────┐     ┌─────────────────┐
│ 脱敏执行    │ ──> │ 遮蔽/替换/加密   │
└──────┬──────┘     └─────────────────┘
       │
       ▼
┌─────────────┐     ┌─────────────────┐
│ 映射记录    │ ──> │ 写入映射表(加密存储)│
└──────┬──────┘     └─────────────────┘
       │
       ▼
┌─────────────┐
│ 审计记录    │ ─── 写入审计日志
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ 结果输出    │ ─── 返回脱敏后文件
└─────────────┘
```

### 5.5 数据流向

```
                    外部数据
                       │
                       ▼
                  ┌─────────┐
                  │ 入口校验 │ ─── 权限验证、格式检查
                  └────┬────┘
                       │
                       ▼
                  ┌─────────┐
                  │加密沙箱 │ ─── AES-256 加密存储
                  └────┬────┘
                       │
           ┌───────────┴───────────┐
           │                       │
           ▼                       ▼
      ┌─────────┐            ┌─────────┐
      │ 原文件   │            │ 映射表  │
      │ (加密)   │            │ (加密)  │
      └─────────┘            └─────────┘
           │                       │
           ▼                       ▼
      ┌─────────┐            ┌─────────┐
      │ 解析器   │            │ 实体信息 │
      │         │ ─────────> │         │
      └─────────┘            └─────────┘
           │                       │
           ▼                       │
      ┌─────────┐                  │
      │ 识别引擎 │                  │
      │         │ ──────────────────┘
      └─────────┘
           │
           ▼
      ┌─────────┐
      │ 脱敏处理 │
      │         │ ───> 生成脱敏后文件
      └─────────┘
           │
           ▼
      ┌─────────┐
      │ 审计日志 │ ───> 记录操作痕迹
      └─────────┘
```

---

## 6. 核心能力设计

### 6.1 文件解析引擎

#### 6.1.1 支持的文件格式

| 格式 | 扩展名 | 解析模式 | 备注 |
|------|--------|----------|------|
| PDF | .pdf | 文本提取 + OCR | 文本层直接提取，图片层 OCR 识别 |
| Word | .doc, .docx | 结构化解析 | 保留段落、表格、样式信息 |
| Excel | .xls, .xlsx | Sheet 解析 | 按 Sheet 和单元格解析 |
| PPT | .ppt, .pptx | 幻灯片解析 | 文本框 + 图形内文字 + 备注 |
| TXT | .txt | 纯文本 | 按行解析 |
| 代码 | .py, .js, .go, .java, .ts, .tsx, .jsx | AST 解析 | 支持代码特定语法识别 |
| Markdown | .md | 结构化解析 | 按标题、代码块、段落解析 |
| CSV | .csv | 结构化解析 | 按行列解析 |
| JSON | .json | 结构化解析 | 按键值对解析 |
| XML | .xml | 结构化解析 | 按标签解析 |

#### 6.1.2 解析输出 Schema

```json
{
  "fileId": "uuid-string",
  "fileName": "original_file.pdf",
  "fileType": "pdf",
  "fileSize": 1024000,
  "fileHash": "sha256-hash-string",
  "parseTime": 1234,
  "content": {
    "metadata": {
      "title": "文件标题",
      "author": "作者名",
      "company": "公司名",
      "createdDate": "2025-01-01T00:00:00Z",
      "modifiedDate": "2025-01-15T10:30:00Z",
      "customProperties": {}
    },
    "pages": [
      {
        "pageNumber": 1,
        "width": 595,
        "height": 842,
        "blocks": [
          {
            "blockId": "block_001",
            "blockType": "text|image|table|shape",
            "content": "文本内容或 base64 图片",
            "bbox": {
              "x": 100,
              "y": 200,
              "width": 300,
              "height": 50
            },
            "style": {
              "fontSize": 12,
              "fontFamily": "Arial",
              "bold": false,
              "italic": false,
              "color": "#000000"
            },
            "children": []
          }
        ]
      }
    ],
    "text": "完整的纯文本内容，用于实体识别"
  }
}
```

#### 6.1.3 解析接口定义

**IPC 通道**: `vault:parse`

**请求参数**:

```json
{
  "filePath": "/path/to/file.pdf",
  "options": {
    "extractMetadata": true,
    "extractImages": false,
    "ocrLanguage": "chi_sim+eng",
    "preserveLayout": true
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "parseResult": { /* 解析输出 Schema */ }
  },
  "error": null
}
```

### 6.2 实体识别引擎

#### 6.2.1 双引擎架构

Vault 采用 SLM 引擎 + 规则引擎的双引擎识别架构：

```
                    实体识别请求
                         │
                         ▼
              ┌─────────────────────┐
              │    调度层 (Router)   │
              │  并行分发/结果合并   │
              └──────────┬──────────┘
                         │
           ┌─────────────┴─────────────┐
           │                           │
           ▼                           ▼
    ┌─────────────┐            ┌─────────────┐
    │  SLM 引擎    │            │ 规则引擎    │
    │ (轻量级模型) │            │ (正则+AST)  │
    └──────┬──────┘            └──────┬──────┘
           │                           │
           └─────────────┬─────────────┘
                         │
                         ▼
              ┌─────────────────────┐
              │    合并层 (Merger)   │
              │ 去重/置信度排序     │
              └──────────┬──────────┘
                         │
                         ▼
                    实体识别结果
```

#### 6.2.2 SLM 引擎设计

| 属性 | 规格 |
|------|------|
| 模型参数 | 1B - 1.5B |
| 模型类型 | Text-to-Text NER 模型 |
| 推理方式 | 本地 CPU/GPU 推理 |
| 内存占用 | 约 500MB (CPU) / 1GB (GPU) |
| 启动方式 | 热启动常驻内存 |
| 支持语言 | 中文、英文（可扩展） |

**支持的实体类型 (NER 标签)**:

| 实体类型 | 标签 | 示例 |
|----------|------|------|
| 人名 | PER | 张三、李明、John Smith |
| 机构名 | ORG | 阿里巴巴、招商银行、XX医院 |
| 地点 | LOC | 北京市、深圳市 |
| 金额 | MONEY | 100万元、$50,000、壹佰万元 |
| 日期时间 | TIME | 2025年1月1日、10:30 |
| 金额范围 | MONEY_RANGE | 5000-10000元 |

#### 6.2.3 规则引擎设计

规则引擎用于补充 SLM 引擎无法覆盖的场景：

| 规则类型 | 应用场景 | 示例 |
|----------|----------|------|
| 正则规则 | 结构化敏感信息 | 身份证号、手机号、银行卡号 |
| 字典规则 | 已知敏感词库 | 银行名称列表、产品代码列表 |
| 组合规则 | 多字段组合判断 | 姓名+手机号+地址组合 |
| 上下文规则 | 上下文相关识别 | "身份证："后的数字 |
| AST 规则 | 代码特定语法 | 函数名、类名、API Key |

**内置正则规则库**:

```json
{
  "rules": [
    {
      "ruleId": "ID_CARD_CN",
      "entityType": "id_card",
      "description": "中国大陆身份证号",
      "pattern": "/^[1-9]\\d{5}(19|20)\\d{2}(0[1-9]|1[0-2])(0[1-9]|[12]\\d|3[01])\\d{3}[\\dXx]$/",
      "confidence": 0.95
    },
    {
      "ruleId": "PHONE_CN_MOBILE",
      "entityType": "phone",
      "description": "中国大陆手机号",
      "pattern": "/^1[3-9]\\d{9}$/",
      "confidence": 0.98
    },
    {
      "ruleId": "BANK_CARD_CN",
      "entityType": "bank_card",
      "description": "银行卡号",
      "pattern": "/^\\d{16,19}$/",
      "confidence": 0.90,
      "validator": "luhn"
    },
    {
      "ruleId": "EMAIL",
      "entityType": "email",
      "description": "电子邮箱",
      "pattern": "/^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$/",
      "confidence": 0.98
    },
    {
      "ruleId": "IP_ADDRESS_PRIVATE",
      "entityType": "private_ip",
      "description": "内网IP地址",
      "pattern": "/^(10\\.\\d{1,3}|172\\.(1[6-9]|2\\d|3[01])|192\\.168)\\.\\d{1,3}\\.\\d{1,3}$/",
      "confidence": 1.0
    },
    {
      "ruleId": "API_KEY_COMMON",
      "entityType": "api_key",
      "description": "常见API Key格式",
      "pattern": "/^(sk-|api-|key-)[a-zA-Z0-9]{20,}$/",
      "confidence": 0.95
    }
  ]
}
```

#### 6.2.4 代码场景 AST 规则

```json
{
  "codeRules": [
    {
      "ruleId": "PYTHON_PRIVATE_FUNC",
      "entityType": "private_function",
      "language": "python",
      "pattern": "def _\\w+\\(.*?\\)",
      "description": "Python私有函数"
    },
    {
      "ruleId": "JS_PRIVATE_VAR",
      "entityType": "private_variable",
      "language": "javascript",
      "pattern": "const (_\\w+|\\w+Priv(ate)?)",
      "description": "JavaScript私有变量"
    },
    {
      "ruleId": "API_KEY_ASSIGNMENT",
      "entityType": "api_key_assignment",
      "language": "*",
      "pattern": "(api_key|apiKey|API_KEY)\\s*=\\s*['\"]([^'\"]{20,})['\"]",
      "description": "API Key赋值语句"
    }
  ]
}
```

#### 6.2.5 实体识别输出 Schema

```json
{
  "requestId": "uuid-string",
  "fileId": "file-uuid",
  "model": "hybrid",
  "entities": [
    {
      "entityId": "ent_001",
      "entityType": "id_card",
      "value": "310101199001011234",
      "span": {
        "start": 120,
        "end": 135,
        "page": 1,
        "blockId": "block_003"
      },
      "confidence": 0.97,
      "source": "rule",
      "ruleId": "ID_CARD_CN",
      "riskLevel": "high"
    },
    {
      "entityId": "ent_002",
      "entityType": "person_name",
      "value": "张三",
      "span": {
        "start": 50,
        "end": 52,
        "page": 1,
        "blockId": "block_001"
      },
      "confidence": 0.89,
      "source": "slm",
      "modelName": "vault-ner-1b",
      "riskLevel": "medium"
    }
  ],
  "statistics": {
    "totalEntities": 15,
    "byType": {
      "id_card": 3,
      "person_name": 5,
      "bank_card": 2,
      "phone": 3,
      "api_key": 1,
      "private_ip": 1
    },
    "byRiskLevel": {
      "high": 6,
      "medium": 5,
      "low": 4
    }
  },
  "processingTime": 850
}
```

#### 6.2.6 实体识别接口定义

**IPC 通道**: `vault:recognize`

**请求参数**:

```json
{
  "fileId": "file-uuid",
  "content": "待识别的文本内容",
  "options": {
    "engine": "hybrid",
    "entityTypes": ["id_card", "person_name", "bank_card", "phone"],
    "minConfidence": 0.70,
    "language": "zh"
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": { /* 实体识别输出 Schema */ },
  "error": null
}
```

### 6.3 脱敏执行引擎

#### 6.3.1 脱敏策略配置 Schema

```json
{
  "policyId": "policy_financial_bank_v1",
  "policyName": "金融行业-银行标准",
  "policyVersion": "1.0.0",
  "description": "适用于银行金融场景的标准脱敏策略",
  "category": "financial",
  "isDefault": true,
  "entityRules": [
    {
      "entityType": "id_card",
      "riskLevel": "high",
      "maskConfig": {
        "mode": "encrypt",
        "preserveFormat": true,
        "preserveLength": true,
        "maskChar": "*",
        "visiblePositions": {
          "prefix": 3,
          "suffix": 4
        }
      },
      "reversible": false,
      "auditRequired": true
    },
    {
      "entityType": "bank_card",
      "riskLevel": "high",
      "maskConfig": {
        "mode": "replace",
        "preserveFormat": true,
        "placeholder": "[银行卡]",
        "keepCardType": true
      },
      "reversible": true,
      "reversibleWithAudit": true
    },
    {
      "entityType": "person_name",
      "riskLevel": "medium",
      "maskConfig": {
        "mode": "replace",
        "placeholderType": "sequential",
        "placeholderTemplate": "客户%s",
        "startIndex": 1
      },
      "reversible": true,
      "reversibleWithAudit": false
    },
    {
      "entityType": "phone",
      "riskLevel": "medium",
      "maskConfig": {
        "mode": "mask",
        "maskChar": "*",
        "visiblePositions": {
          "prefix": 3,
          "suffix": 4
        }
      },
      "reversible": false
    },
    {
      "entityType": "address",
      "riskLevel": "medium",
      "maskConfig": {
        "mode": "partial",
        "preservePrefix": 6,
        "maskRemaining": true
      },
      "reversible": false
    },
    {
      "entityType": "api_key",
      "riskLevel": "critical",
      "maskConfig": {
        "mode": "replace",
        "placeholder": "[REDACTED_API_KEY]"
      },
      "reversible": false
    },
    {
      "entityType": "private_ip",
      "riskLevel": "low",
      "maskConfig": {
        "mode": "replace",
        "placeholder": "[内网IP]"
      },
      "reversible": false
    }
  ],
  "globalSettings": {
    "preserveLayout": true,
    "preserveNumbers": false,
    "preserveUrls": false,
    "ignorePatterns": ["^测试.*$", ".*[Tt]est.*"]
  }
}
```

#### 6.3.2 脱敏模式说明

| 模式 | 说明 | 适用场景 | 示例 |
|------|------|----------|------|
| mask | 字符遮蔽 | 一般敏感信息 | 310101199001011234 -> 310***********1234 |
| replace | 字符串替换 | 需要可读性的场景 | "张三" -> "客户A" |
| encrypt | 格式保留加密 | 需要可逆还原的场景 | 使用 FPE 算法加密 |
| delete | 完全删除 | 高风险且无需保留的结构 | 完整删除某字段 |
| partial | 部分保留 | 地址等长文本 | "北京市朝阳区XX路1号" -> "北京市朝阳区" |

#### 6.3.3 脱敏接口定义

**IPC 通道**: `vault:mask`

**请求参数**:

```json
{
  "fileId": "file-uuid",
  "entities": [ /* 实体列表 */ ],
  "policy": { /* 脱敏策略配置 */ },
  "options": {
    "preserveLayout": true,
    "outputFormat": "same_as_input",
    "generateReport": true
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "maskedFileId": "masked-file-uuid",
    "maskedContent": "脱敏后的内容",
    "maskedFilePath": "/sandbox/path/to/masked_file",
    "statistics": {
      "totalEntities": 15,
      "maskedEntities": 15,
      "byType": {
        "id_card": 3,
        "person_name": 5
      },
      "processingTime": 1200
    },
    "mappingId": "mapping-uuid"
  },
  "error": null
}
```

### 6.4 FPE 可逆还原引擎

#### 6.4.1 FPE 算法设计

Format-Preserving Encryption (格式保留加密) 用于需要可逆还原的场景：

- 输出格式与输入格式一致（如 16 位数字加密后仍是 16 位数字）
- 使用 AES-256 算法作为底层加密
- 支持带密钥的确定性加密（相同输入产生相同输出，便于 AI 处理）

#### 6.4.2 FPE 密钥管理

| 组件 | 说明 |
|------|------|
| 主密钥 (MEK) | AES-256 密钥，用于加密映射表 |
| FPE 密钥 | 由主密钥派生，用于 FPE 加密实体值 |
| 密钥派生 | 使用 HKDF 从主密钥派生 FPE 密钥 |
| 密钥轮换 | 支持定期轮换，保留历史密钥用于解密历史数据 |

#### 6.4.3 映射表结构

```json
{
  "mappingId": "mapping_uuid",
  "mappingVersion": "v1",
  "createdAt": "2025-01-01T10:00:00Z",
  "policyId": "policy_financial_bank_v1",
  "entities": [
    {
      "entityId": "ent_001",
      "originalValue": "310101199001011234",
      "maskedValue": "enc_7f8a9b0c1d2e3f4a",
      "encryptionInfo": {
        "algorithm": "AES-256-FPE",
        "keyVersion": "v1",
        "iv": "random_iv_value"
      }
    }
  ],
  "metadata": {
    "fileId": "original_file_uuid",
    "maskedFileId": "masked_file_uuid",
    "operator": "user_id",
    "operation": "mask"
  }
}
```

#### 6.4.4 还原接口定义

**IPC 通道**: `vault:restore`

**请求参数**:

```json
{
  "mappingId": "mapping-uuid",
  "entityIds": ["ent_001", "ent_002"],
  "options": {
    "requireVerification": true,
    "verificationMethod": "pin"
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "restoredEntities": [
      {
        "entityId": "ent_001",
        "originalValue": "310101199001011234"
      }
    ],
    "auditRecordId": "audit_uuid"
  },
  "error": null
}
```

### 6.5 本地加密沙箱

#### 6.5.1 沙箱架构

```
┌─────────────────────────────────────────────────────────────┐
│                    应用程序层 (Desktop等)                    │
│                     (无法直接访问沙箱)                        │
└────────────────────────────┬────────────────────────────────┘
                             │ Vault SDK / IPC
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                    沙箱访问控制层                              │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐             │
│  │  权限校验   │  │  凭证验证   │  │  配额管理   │             │
│  └────────────┘  └────────────┘  └────────────┘             │
└────────────────────────────┬────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                    加密文件系统层                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              AES-256 加密存储                         │    │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐             │    │
│  │  │原文件    │  │脱敏文件  │  │  缓存    │             │    │
│  │  │(加密)   │  │(加密)   │  │ (加密)  │             │    │
│  │  └─────────┘  └─────────┘  └─────────┘             │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                    密钥管理层                                │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐             │
│  │  主密钥    │  │  派生密钥  │  │  密钥存储  │             │
│  └────────────┘  └────────────┘  └────────────┘             │
└─────────────────────────────────────────────────────────────┘
```

#### 6.5.2 沙箱访问控制

| 控制维度 | 说明 | 实现方式 |
|----------|------|----------|
| 进程隔离 | 只有 Vault 进程可以访问沙箱文件 | 文件系统权限 (chmod 700) |
| 凭证验证 | 用户需要验证身份后才能访问 | 口令/PIN/生物识别 |
| 二次验证 | 高风险操作需要二次确认 | 验证码/硬件令牌 |
| 操作审计 | 所有访问操作记录到审计日志 | 审计引擎 |
| 配额限制 | 单用户/单文件大小限制 | 配额管理模块 |

#### 6.5.3 口令管理

```json
{
  "passwordPolicy": {
    "minLength": 8,
    "requireUppercase": true,
    "requireLowercase": true,
    "requireDigit": true,
    "requireSpecial": false,
    "maxAge": 0,
    "historyCount": 5
  },
  "verificationMethods": [
    "password",
    "pin",
    "biometric"
  ],
  "lockout": {
    "maxAttempts": 5,
    "lockoutDuration": 300,
    "unlockWithAdmin": true
  }
}
```

#### 6.5.4 沙箱接口定义

**IPC 通道**: `vault:sandbox`

**请求参数**:

```json
{
  "action": "write|read|delete|list",
  "fileId": "optional-file-uuid",
  "data": "base64-encoded-content",
  "credentials": {
    "method": "password|pin|biometric",
    "token": "credential-token"
  }
}
```

**响应参数**:

```json
{
  "success": true,
  "data": {
    "fileId": "uuid",
    "filePath": "/sandbox/xxx",
    "fileSize": 1024,
    "fileHash": "sha256"
  },
  "error": null
}
```

### 6.6 元数据脱敏

#### 6.6.1 支持的元数据类型

| 文件类型 | 可脱敏元数据 |
|----------|--------------|
| PDF | 作者、公司名、创建者、标题、主题、关键词、修改历史、自定义属性 |
| Word | 作者、公司名、经理、标题、主题、关键词、评论修订、隐藏文字 |
| Excel | 作者、公司名、经理、标题、批注、自定义属性、隐藏Sheet |
| PPT | 作者、公司名、标题、备注、隐藏幻灯片 |

#### 6.6.2 元数据脱敏规则

```json
{
  "metadataRules": [
    {
      "fieldName": "author",
      "action": "replace",
      "replacement": "[文档创建者]",
      "preserveOriginal": false
    },
    {
      "fieldName": "company",
      "action": "remove",
      "preserveOriginal": false
    },
    {
      "fieldName": "comments",
      "action": "remove",
      "preserveOriginal": false
    },
    {
      "fieldName": "revisionHistory",
      "action": "remove",
      "preserveOriginal": false
    },
    {
      "fieldName": "customProperties",
      "action": "remove",
      "preserveOriginal": false
    }
  ]
}
```

### 6.7 审计日志

#### 6.7.1 审计日志 Schema

```json
{
  "logId": "log_uuid",
  "logType": "mask|restore|export|config|auth|error",
  "timestamp": "2025-01-01T10:30:00.123Z",
  "operator": {
    "userId": "user_uuid",
    "userName": "zhangsan",
    "ipAddress": "127.0.0.1",
    "deviceId": "device_uuid"
  },
  "operation": {
    "action": "file_mask",
    "fileId": "file_uuid",
    "fileName": "contract.pdf",
    "fileHash": "sha256",
    "policyId": "policy_uuid",
    "policyName": "金融行业-银行标准",
    "parameters": {}
  },
  "result": {
    "status": "success|partial_failure|failure",
    "entitiesProcessed": 15,
    "entitiesFailed": 0,
    "processingTime": 1234
  },
  "risk": {
    "highRiskEntities": 3,
    "restoredEntities": 0,
    "requiresReview": false
  },
  "signature": {
    "algorithm": "SHA-256",
    "value": "signature_hash",
    "keyId": "signing_key_id"
  }
}
```

#### 6.7.2 审计日志接口定义

**IPC 通道**: `vault:audit`

**查询请求**:

```json
{
  "action": "query",
  "filters": {
    "startTime": "2025-01-01T00:00:00Z",
    "endTime": "2025-01-31T23:59:59Z",
    "logTypes": ["mask", "restore"],
    "userIds": ["user_uuid"],
    "fileIds": ["file_uuid"],
    "status": "success"
  },
  "pagination": {
    "page": 1,
    "pageSize": 100
  },
  "sort": {
    "field": "timestamp",
    "order": "desc"
  }
}
```

**导出请求**:

```json
{
  "action": "export",
  "filters": { /* 同上 */ },
  "format": "csv|json|xml",
  "includeSignature": true,
  "encryption": {
    "enabled": false,
    "password": "optional-password"
  }
}
```

### 6.8 模板系统

#### 6.8.1 预置行业模板

**金融行业-银行标准**

```json
{
  "templateId": "tpl_financial_bank",
  "templateName": "金融行业-银行标准",
  "category": "financial",
  "industry": "banking",
  "description": "适用于银行金融场景，覆盖客户隐私、账户信息、交易数据等",
  "isBuiltIn": true,
  "entityRules": [
    {"entityType": "id_card", "maskMode": "encrypt", "reversible": false},
    {"entityType": "bank_card", "maskMode": "replace", "reversible": true},
    {"entityType": "bank_account", "maskMode": "replace", "reversible": true},
    {"entityType": "person_name", "maskMode": "sequential", "reversible": true},
    {"entityType": "phone", "maskMode": "mask", "reversible": false},
    {"entityType": "address", "maskMode": "partial", "reversible": false},
    {"entityType": "money", "maskMode": "preserve", "reversible": false},
    {"entityType": "org_name", "maskMode": "preserve", "reversible": false}
  ],
  "metadataRules": [
    {"fieldName": "author", "action": "remove"},
    {"fieldName": "company", "action": "remove"}
  ]
}
```

**医疗行业-病历标准**

```json
{
  "templateId": "tpl_medical_medical_record",
  "templateName": "医疗行业-病历标准",
  "category": "medical",
  "industry": "healthcare",
  "description": "适用于医疗机构病历处理，保护患者隐私同时保留医疗数据",
  "isBuiltIn": true,
  "entityRules": [
    {"entityType": "patient_name", "maskMode": "sequential", "reversible": true},
    {"entityType": "id_card", "maskMode": "partial", "reversible": false},
    {"entityType": "phone", "maskMode": "mask", "reversible": false},
    {"entityType": "address", "maskMode": "partial", "reversible": false},
    {"entityType": "case_number", "maskMode": "replace", "reversible": true},
    {"entityType": "diagnosis", "maskMode": "preserve", "reversible": false},
    {"entityType": "lab_result", "maskMode": "preserve", "reversible": false}
  ],
  "metadataRules": [
    {"fieldName": "patientId", "action": "remove"},
    {"fieldName": "author", "action": "replace"}
  ]
}
```

**法律行业-案件标准**

```json
{
  "templateId": "tpl_legal_case",
  "templateName": "法律行业-案件标准",
  "category": "legal",
  "industry": "law",
  "description": "适用于法律文书处理，保护当事人隐私同时保留案件信息",
  "isBuiltIn": true,
  "entityRules": [
    {"entityType": "person_name", "maskMode": "sequential", "reversible": true},
    {"entityType": "id_card", "maskMode": "encrypt", "reversible": false},
    {"entityType": "phone", "maskMode": "mask", "reversible": false},
    {"entityType": "address", "maskMode": "partial", "reversible": false},
    {"entityType": "case_number", "maskMode": "preserve", "reversible": false},
    {"entityType": "court_name", "maskMode": "preserve", "reversible": false},
    {"entityType": "lawyer_name", "maskMode": "preserve", "reversible": false},
    {"entityType": "contract_number", "maskMode": "replace", "reversible": true}
  ]
}
```

**通用标准**

```json
{
  "templateId": "tpl_general_standard",
  "templateName": "通用标准",
  "category": "general",
  "industry": "common",
  "description": "适用于一般办公场景的通用脱敏规则",
  "isBuiltIn": true,
  "entityRules": [
    {"entityType": "person_name", "maskMode": "sequential", "reversible": true},
    {"entityType": "phone", "maskMode": "mask", "reversible": false},
    {"entityType": "email", "maskMode": "replace", "reversible": false},
    {"entityType": "address", "maskMode": "partial", "reversible": false},
    {"entityType": "id_card", "maskMode": "mask", "reversible": false}
  ]
}
```

#### 6.8.2 角色适配推荐

| 角色 | 推荐模板 | 说明 |
|------|----------|------|
| 柜员/客户经理 | 金融-银行标准 | 覆盖客户信息、账户信息 |
| 风控/审计人员 | 金融-银行标准(严格) | 高风险实体不可逆 |
| 信贷审批 | 金融-银行标准+金额保留 | 保留金额用于审批 |
| 医生/护士 | 医疗-病历标准 | 保留诊断和检验数据 |
| 医院信息科 | 医疗-病历标准 | 管理员视角 |
| 律师 | 法律-案件标准 | 保留案件相关信息 |
| 法官/检察官 | 法律-案件标准(严格) | 高敏感案件处理 |
| 普通员工 | 通用标准 | 日常办公文档 |

#### 6.8.3 模板管理接口

**IPC 通道**: `vault:config`

**查询模板列表**:

```json
{
  "action": "listTemplates",
  "filters": {
    "category": "financial",
    "isBuiltIn": true
  }
}
```

**应用模板**:

```json
{
  "action": "applyTemplate",
  "templateId": "tpl_financial_bank",
  "options": {
    "overrides": [
      {
        "entityType": "phone",
        "maskMode": "replace",
        "reversible": true
      }
    ]
  }
}
```

### 6.9 企业级能力增强

#### 6.9.1 自动化能力

- 文件夹监控（Watch Folder）：监控指定目录，新文件自动触发脱敏
- 定时任务：按 Cron 表达式定时执行批量脱敏
- 事件驱动：与 FileBay/Desktop 联动，文件上传事件自动触发
- Pipeline 编排：多步脱敏流程（解析->识别->脱敏->加密->归档）可编排

#### 6.9.2 批量处理能力

- 批量文件脱敏：支持整个文件夹/多级目录递归处理
- 并行处理：多文件并行脱敏（可配置并发数）
- 进度追踪：批量任务进度报告、失败重试
- 增量处理：仅处理新增/变更文件（基于文件 hash）

#### 6.9.3 可扩展性

- 插件式规则引擎：支持自定义实体识别插件（Python/JS）
- 自定义脱敏策略：用户可编写自定义脱敏函数
- 行业扩展包：金融/医疗/法律/政务等行业规则包，可独立安装
- 国际化实体支持：支持多语言实体识别（中文/英文/日文）

#### 6.9.4 API 开放性

- RESTful API：供第三方系统集成（企业级）
- CLI 工具：命令行脱敏工具（供 CI/CD 集成）
- SDK：Python/Node.js SDK（供开发者集成）
- Webhook：脱敏完成后回调通知

API 端点示例：

```
POST /api/v1/vault/mask          # 单文件脱敏
POST /api/v1/vault/mask/batch    # 批量脱敏
POST /api/v1/vault/restore       # 还原
GET  /api/v1/vault/tasks/:id     # 查询任务状态
GET  /api/v1/vault/templates     # 获取模板列表
POST /api/v1/vault/rules         # 创建自定义规则
GET  /api/v1/vault/audit         # 查询审计日志
```

---

## 7. API/SDK 设计

### 7.1 IPC 通道总览

| 通道名称 | 功能 | 调用方 |
|----------|------|--------|
| `vault:parse` | 文件解析 | Desktop, Desktop 服务端（Dify）, FileBay |
| `vault:recognize` | 实体识别 | Desktop, Desktop 服务端（Dify）, FileBay |
| `vault:mask` | 执行脱敏 | Desktop, Desktop 服务端（Dify）, FileBay |
| `vault:restore` | 还原脱敏 | Desktop |
| `vault:audit` | 审计日志 | Desktop |
| `vault:config` | 配置管理 | Desktop |
| `vault:sandbox` | 沙箱操作 | Desktop, Desktop 服务端（Dify） |
| `vault:health` | 健康检查 | Desktop |
| `vault:init` | 初始化 | Desktop |

### 7.2 SDK 接口设计

#### 7.2.1 SDK 初始化

```typescript
// TypeScript SDK 示例
import { VaultClient } from '@cheersai/vault-sdk';

const vault = new VaultClient({
  appId: 'desktop',
  appKey: 'app_key_here',
  endpoint: 'unix:///var/run/vault.sock',
  timeout: 30000,
  retry: {
    maxAttempts: 3,
    backoff: 'exponential'
  }
});

// 初始化连接
await vault.initialize();

// 健康检查
const health = await vault.health();
console.log(`Vault status: ${health.status}`);
```

#### 7.2.2 文件脱敏完整流程

```typescript
// 完整文件脱敏流程
async function maskFile(filePath: string, templateId: string) {
  // Step 1: 解析文件
  const parseResult = await vault.parse(filePath, {
    extractMetadata: true,
    preserveLayout: true
  });
  
  // Step 2: 实体识别
  const recognizeResult = await vault.recognize(parseResult.fileId, {
    content: parseResult.content.text,
    entityTypes: ['person_name', 'id_card', 'phone', 'bank_card']
  });
  
  // Step 3: 应用脱敏策略
  const maskResult = await vault.mask(parseResult.fileId, {
    entities: recognizeResult.entities,
    templateId: templateId
  });
  
  // Step 4: 获取脱敏后文件
  return {
    fileId: maskResult.maskedFileId,
    filePath: maskResult.maskedFilePath,
    statistics: maskResult.statistics
  };
}
```

#### 7.2.3 SDK 错误码

| 错误码 | 含义 | 处理建议 |
|--------|------|----------|
| VAULT_001 | 文件格式不支持 | 检查文件扩展名 |
| VAULT_002 | 文件解析失败 | 尝试重新保存文件 |
| VAULT_003 | 文件过大 | 拆分文件或调整大小限制 |
| VAULT_004 | 实体识别失败 | 检查内容是否为空 |
| VAULT_005 | 脱敏策略不存在 | 检查策略ID |
| VAULT_006 | 映射表不存在 | 文件可能未脱敏 |
| VAULT_007 | 还原权限不足 | 联系管理员 |
| VAULT_008 | 沙箱访问被拒绝 | 验证用户凭证 |
| VAULT_009 | 审计日志写入失败 | 检查存储空间 |
| VAULT_010 | 密钥不存在 | 重新初始化 Vault |
| VAULT_011 | 网络通信失败 | 检查 Vault 服务状态 |
| VAULT_012 | 超时 | 增加超时时间 |

### 7.3 IPC 协议定义

#### 7.3.1 消息格式

```json
{
  "header": {
    "version": "1.0",
    "channel": "vault:mask",
    "requestId": "uuid",
    "timestamp": 1706745600000,
    "appId": "desktop",
    "authToken": "jwt_token"
  },
  "body": {
    "fileId": "file_uuid",
    "entities": [],
    "policy": {}
  }
}
```

#### 7.3.2 响应格式

```json
{
  "header": {
    "version": "1.0",
    "requestId": "uuid",
    "timestamp": 1706745600100,
    "status": "success"
  },
  "body": {
    "data": {},
    "error": null
  }
}
```

---

## 8. 数据模型

### 8.1 映射表结构

```sql
-- 映射表主表
CREATE TABLE mapping (
    mapping_id VARCHAR(36) PRIMARY KEY,
    mapping_version VARCHAR(10) NOT NULL,
    created_at TIMESTAMP NOT NULL,
    policy_id VARCHAR(36) NOT NULL,
    file_id VARCHAR(36) NOT NULL,
    operator_id VARCHAR(36) NOT NULL,
    encrypted_data BLOB NOT NULL,  -- 加密的实体映射数据
    signature VARCHAR(256) NOT NULL,
    INDEX idx_file_id (file_id),
    INDEX idx_created_at (created_at)
);

-- 映射表明细（加密存储）
CREATE TABLE mapping_entry (
    entry_id VARCHAR(36) PRIMARY KEY,
    mapping_id VARCHAR(36) NOT NULL,
    entity_type VARCHAR(50) NOT NULL,
    original_value_hash VARCHAR(64) NOT NULL,  -- 原始值哈希
    masked_value VARCHAR(500) NOT NULL,
    reversible BOOLEAN NOT NULL DEFAULT FALSE,
    risk_level VARCHAR(10) NOT NULL,
    FOREIGN KEY (mapping_id) REFERENCES mapping(mapping_id)
);
```

### 8.2 审计日志结构

```sql
-- 审计日志表
CREATE TABLE audit_log (
    log_id VARCHAR(36) PRIMARY KEY,
    log_type VARCHAR(20) NOT NULL,  -- mask, restore, export, config, auth, error
    timestamp TIMESTAMP(3) NOT NULL,
    operator_id VARCHAR(36) NOT NULL,
    operator_name VARCHAR(100),
    action VARCHAR(50) NOT NULL,
    file_id VARCHAR(36),
    file_name VARCHAR(255),
    file_hash VARCHAR(64),
    policy_id VARCHAR(36),
    result_status VARCHAR(20) NOT NULL,
    entities_processed INT,
    processing_time_ms INT,
    ip_address VARCHAR(45),
    device_id VARCHAR(36),
    additional_data JSON,
    signature VARCHAR(256) NOT NULL,
    INDEX idx_timestamp (timestamp),
    INDEX idx_operator (operator_id),
    INDEX idx_file (file_id),
    INDEX idx_log_type (log_type)
);
```

### 8.3 规则配置结构

```sql
-- 脱敏策略表
CREATE TABLE masking_policy (
    policy_id VARCHAR(36) PRIMARY KEY,
    policy_name VARCHAR(100) NOT NULL,
    policy_version VARCHAR(10) NOT NULL,
    category VARCHAR(50),
    description TEXT,
    is_built_in BOOLEAN NOT NULL DEFAULT FALSE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    created_by VARCHAR(36),
    config_data JSON NOT NULL  -- 策略配置JSON
);

-- 实体规则表
CREATE TABLE entity_rule (
    rule_id VARCHAR(36) PRIMARY KEY,
    entity_type VARCHAR(50) NOT NULL,
    rule_name VARCHAR(100),
    pattern TEXT,  -- 正则表达式或规则定义
    mask_mode VARCHAR(20) NOT NULL,
    reversible BOOLEAN NOT NULL DEFAULT FALSE,
    risk_level VARCHAR(10) NOT NULL,
    confidence DECIMAL(5,4),
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);

-- 模板表
CREATE TABLE template (
    template_id VARCHAR(36) PRIMARY KEY,
    template_name VARCHAR(100) NOT NULL,
    category VARCHAR(50),
    industry VARCHAR(50),
    description TEXT,
    is_built_in BOOLEAN NOT NULL DEFAULT FALSE,
    config_data JSON NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
```

### 8.4 沙箱文件索引

```sql
-- 沙箱文件索引表
CREATE TABLE sandbox_file (
    file_id VARCHAR(36) PRIMARY KEY,
    original_name VARCHAR(255) NOT NULL,
    encrypted_path VARCHAR(500) NOT NULL,
    file_size BIGINT NOT NULL,
    file_hash VARCHAR(64) NOT NULL,
    mime_type VARCHAR(100),
    encryption_key_id VARCHAR(36),
    status VARCHAR(20) NOT NULL,  -- active, deleted, archived
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    accessed_at TIMESTAMP,
    owner_id VARCHAR(36) NOT NULL,
    INDEX idx_owner (owner_id),
    INDEX idx_status (status),
    INDEX idx_created (created_at)
);

-- 文件生命周期管理
CREATE TABLE file_lifecycle (
    lifecycle_id VARCHAR(36) PRIMARY KEY,
    file_id VARCHAR(36) NOT NULL,
    operation VARCHAR(20) NOT NULL,  -- create, read, write, delete, restore
    operator_id VARCHAR(36),
    timestamp TIMESTAMP NOT NULL,
    details JSON,
    FOREIGN KEY (file_id) REFERENCES sandbox_file(file_id)
);
```

---

## 9. 安全设计

### 9.1 加密方案

#### 9.1.1 加密层级

| 层级 | 加密内容 | 算法 | 密钥类型 |
|------|----------|------|----------|
| 存储加密 | 文件内容、映射表 | AES-256-GCM | 数据加密密钥 (DEK) |
| 密钥加密 | DEK 本身 | AES-256-GCM | 密钥加密密钥 (KEK) |
| 传输加密 | IPC 通信 | TLS 1.3 | 临时会话密钥 |
| 审计签名 | 审计日志完整性 | ECDSA P-256 | 签名私钥 |

#### 9.1.2 密钥体系架构

```
                    ┌─────────────────┐
                    │   用户主口令     │
                    │ (用户输入)       │
                    └────────┬────────┘
                             │
                             ▼ PBKDF2 (100000 iterations)
                    ┌─────────────────┐
                    │   KEK 派生密钥   │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
     ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
     │ 文件DEK     │ │ 映射表DEK   │ │ 审计签名密钥 │
     └─────────────┘ └─────────────┘ └─────────────┘
```

#### 9.1.3 FPE 加密参数

```json
{
  "fpeConfig": {
    "algorithm": "AES-256-FF3-1",
    "tweakMode": "sequence",
    "charset": "alphanumeric",
    "roundLimit": 4,
    "keyRotationDays": 90
  }
}
```

### 9.2 密钥管理

#### 9.2.1 本地密钥存储

| 密钥类型 | 存储位置 | 保护方式 |
|----------|----------|----------|
| 主 KEK | Keychain (macOS) / Credential Manager (Windows) | OS 级保护 |
| 签名私钥 | HSM 模拟 (软件 Keystore) | PIN 保护 |
| 会话密钥 | 内存 | 不持久化 |

#### 9.2.2 密钥生命周期

- **生成**：首次初始化时生成，使用 OS 安全随机数
- **存储**：主密钥存储于 OS 凭证管理器，映射表 DEK 由主密钥加密后存储
- **使用**：运行时解密到内存，使用后立即清除内存
- **轮换**：支持手动触发轮换，保留旧密钥用于解密历史数据
- **销毁**：删除操作不可逆，销毁前需确认无依赖数据

#### 9.2.3 可替换密钥管理

Vault 支持多种密钥管理方案：

```json
{
  "keyManagement": {
    "default": "local",
    "supported": [
      {
        "type": "local",
        "description": "本地密钥存储 (默认)",
        "implementation": "OS Keychain/Credential Manager"
      },
      {
        "type": "hashicorp_vault",
        "description": "HashiCorp Vault",
        "endpoint": "https://vault.example.com:8200",
        "authMethod": "kubernetes",
        "secretPath": "secret/data/cheersai/vault"
      },
      {
        "type": "cloud_kms",
        "description": "云 KMS (AWS/GCP/Azure)",
        "provider": "aws",
        "keyId": "arn:aws:kms:region:account:key/key-id"
      },
      {
        "type": "hsm",
        "description": "硬件安全模块",
        "device": "pkcs11:/usr/lib/softhsm/libsofthsm2.so",
        "slot": 0
      }
    ]
  }
}
```

### 9.3 访问控制

#### 9.3.1 权限模型

采用 RBAC (基于角色的访问控制) 模型：

| 角色 | 权限 | 说明 |
|------|------|------|
| admin | 全部权限 | 系统管理员 |
| security_officer | 策略管理、审计查看 | 安全负责人 |
| operator | 文件脱敏、还原 | 普通操作员 |
| auditor | 仅审计查看 | 审计人员 |
| user | 文件上传、脱敏使用 | 普通用户 |

#### 9.3.2 细粒度权限

```json
{
  "permissions": [
    "vault:file:read",
    "vault:file:write",
    "vault:file:delete",
    "vault:mask:execute",
    "vault:restore:execute",
    "vault:restore:high_risk",
    "vault:policy:read",
    "vault:policy:write",
    "vault:audit:read",
    "vault:audit:export",
    "vault:config:read",
    "vault:config:write"
  ],
  "rolePermissions": {
    "admin": "*",
    "security_officer": ["vault:policy:*", "vault:audit:*"],
    "operator": ["vault:file:*", "vault:mask:execute", "vault:restore:execute"],
    "auditor": ["vault:audit:read"],
    "user": ["vault:file:read", "vault:mask:execute"]
  }
}
```

### 9.4 合规适配

#### 9.4.1 等保 2.0 三级要求覆盖

| 等保要求 | Vault 实现 | 验证方式 |
|----------|------------|----------|
| 身份鉴别 | 用户凭证 + 多因素认证 | PIN/生物识别 |
| 访问控制 | RBAC 权限模型 | 权限矩阵测试 |
| 安全审计 | 完整审计日志 + 签名防篡改 | 日志完整性校验 |
| 数据完整性 | SHA-256 哈希 + ECDSA 签名 | 签名验证 |
| 数据保密性 | AES-256-GCM 加密存储 | 加密强度评估 |
| 重要数据保护 | 沙箱隔离 + 密钥管理 | 渗透测试 |
| 密钥管理 | 分层密钥 + KEK/DEK | 密钥管理审计 |
| 备份恢复 | 本地备份 + 导出加密 | 恢复测试 |

#### 9.4.2 GDPR 合规

| GDPR 要求 | Vault 实现 |
|-----------|------------|
| 数据最小化 | Span-level 精确脱敏，避免过度处理 |
| 目的限制 | 仅用于脱敏，不用于其他目的 |
| 存储限制 | 映射表可配置自动过期 |
| 安全处理 | 加密 + 访问控制 |
| 数据主体权利 | 支持还原（受控） |

#### 9.4.3 HIPAA 合规

| HIPAA 要求 | Vault 实现 |
|------------|------------|
| PHI 保护 | 自动识别并脱敏医疗实体 |
| 访问控制 | 最小权限原则 |
| 审计追踪 | 完整操作记录 |
| 加密 | AES-256 存储加密 |
| 密钥管理 | 安全密钥存储 |

### 9.5 映射表安全设计

#### 9.5.1 威胁模型

- 威胁1：映射表文件被拷贝 -> 攻击者可还原脱敏数据
- 威胁2：映射表和脱敏文件同时泄露 -> 数据完全暴露
- 威胁3：内存中映射表被 dump -> 敏感数据泄露

#### 9.5.2 分离存储策略

- 映射表和脱敏文件必须分离存储
- 映射表存储在加密沙箱内，脱敏文件可存储在沙箱外
- 上传到 FileBay 的文件不包含映射信息

#### 9.5.3 映射表加密方案

- 主密钥（Master Key）：用户口令派生（PBKDF2, 100000 轮, SHA-256）
- 文件密钥（File Key）：每个映射表独立密钥（随机生成）
- 文件密钥用主密钥加密后存储
- 加密算法：AES-256-GCM（带认证加密）
- 密钥层级：User Password -> PBKDF2 -> Master Key -> 加密 File Key -> 加密 Mapping Data

#### 9.5.4 映射表文件格式

```json
{
  "version": "1.0",
  "algorithm": "AES-256-GCM",
  "kdf": "PBKDF2-SHA256",
  "kdf_iterations": 100000,
  "salt": "<base64>",
  "encrypted_file_key": "<base64>",
  "iv": "<base64>",
  "auth_tag": "<base64>",
  "encrypted_data": "<base64>",
  "source_file_hash": "<sha256>",
  "created_at": "ISO8601",
  "expires_at": "ISO8601"
}
```

#### 9.5.5 防泄露措施

- 映射表有效期：可设置自动过期（默认 30 天）
- 访问次数限制：可设置最大还原次数
- 硬件绑定：映射表可绑定到特定设备（Machine ID）
- 自毁机制：检测到异常访问时自动销毁映射表
- 内存保护：映射数据使用后立即从内存清除（secure_zero）

### 9.6 安全接口定义

**IPC 通道**: `vault:security`

**密钥轮换请求**:

```json
{
  "action": "rotateKeys",
  "options": {
    "keyTypes": ["file_dek", "mapping_dek"],
    "preserveOldKeys": true,
    "oldKeyRetentionDays": 30
  }
}
```

**安全状态检查**:

```json
{
  "action": "securityStatus",
  "checks": [
    "encryption",
    "keyIntegrity",
    "accessControl",
    "auditLog",
    "sessionTimeout"
  ]
}
```

---

## 10. 验收标准

### 10.1 功能验收

#### 10.1.1 脱敏准确率

| 指标 | 目标值 | 测量方法 |
|------|--------|----------|
| 召回率 (Recall) | >= 99% | 测试集包含 1000 个已知敏感实体，检测出 >= 990 个 |
| 精确率 (Precision) | >= 95% | 检测出的实体中 >= 95% 为真正的敏感信息 |
| F1 分数 | >= 97% | 2 * Precision * Recall / (Precision + Recall) |

**测试集要求**：

- 覆盖所有支持的实体类型
- 包含至少 100 份真实文档样本
- 包含边界案例和对抗样本

#### 10.1.2 还原完整性

| 指标 | 目标值 | 说明 |
|------|--------|------|
| 还原成功率 | 100% | 在合规前提下，所有可还原实体均可还原 |
| 还原准确性 | 100% | 还原后的值与原始值完全一致 |
| 还原审计覆盖率 | 100% | 每次还原操作均有审计记录 |

#### 10.1.3 格式保持

| 指标 | 目标值 | 说明 |
|------|--------|------|
| 布局保持率 | >= 98% | 脱敏后文件与原文件布局一致性 |
| 格式识别率 | >= 99% | 可正确识别并处理的文档格式 |
| 结构完整性 | 100% | 表格、列表等结构保持完整 |

### 10.2 性能验收

#### 10.2.1 处理速度

| 场景 | 单文件目标 | 说明 |
|------|------------|------|
| 小文件 (< 1MB) | <= 2秒 | 文本类文件 |
| 中文件 (1-10MB) | <= 5秒 | 包含表格的文档 |
| 大文件 (10-100MB) | <= 30秒 | 复杂排版的文档 |
| 超大文件 (> 100MB) | <= 2分钟 | 分批处理 |

#### 10.2.2 并发处理

| 指标 | 目标值 |
|------|--------|
| 队列容量 | >= 100 个并发请求 |
| 平均等待时间 | <= 1秒 |
| 内存峰值 | <= 500MB (不含模型) |

#### 10.2.3 启动性能

| 指标 | 目标值 |
|------|--------|
| 冷启动时间 | <= 3秒 |
| 热启动时间 | <= 500ms |
| 内存常驻 | <= 200MB |

### 10.3 安全验收

#### 10.3.1 加密验证

| 检查项 | 验收标准 |
|--------|----------|
| 加密算法 | AES-256-GCM |
| 密钥长度 | 256 位 |
| IV/Nonce | 随机生成，不可重复 |
| 认证标签 | 128 位 |

#### 10.3.2 数据清除验证

| 检查项 | 验收标准 |
|--------|----------|
| 内存清除 | 敏感数据处理后 1 秒内从内存清除 |
| 临时文件 | 处理完成后 5 秒内删除 |
| 缓存清理 | 退出时清除所有敏感缓存 |

#### 10.3.3 审计日志验证

| 检查项 | 验收标准 |
|--------|----------|
| 日志完整性 | 日志哈希可验证 |
| 不可篡改性 | 日志签名可校验 |
| 时效性 | 时间戳精确到毫秒 |

### 10.4 验收测试用例示例

```
TC-001: 身份证号脱敏测试
  输入: "用户身份证号: 310101199001011234"
  期望: 脱敏后为 "31**************234" 或加密后的不可逆值
  
TC-002: 银行卡号脱敏测试
  输入: "银行账号: 6222021234567890123"
  期望: 脱敏后为 "[银行卡]" 或 "6222**********23" (保留卡种)
  
TC-003: 嵌套敏感信息测试
  输入: "联系人张三，电话13812345678，身份证310101199001011234"
  期望: 三种实体均被识别和脱敏
  
TC-004: 格式保留测试
  输入: PDF文档，包含多页和表格
  期望: 脱敏后PDF保持原有页数和表格结构
  
TC-005: 可逆还原测试
  前提: 已对某实体执行可逆脱敏
  输入: 映射表ID和实体ID
  期望: 还原后的值与原始值完全一致
  
TC-006: 审计日志验证
  输入: 执行脱敏操作
  期望: 审计日志包含完整的操作记录，包含签名
  
TC-007: 权限验证测试
  输入: 无还原权限用户尝试还原高风险实体
  期望: 返回权限不足错误，操作被拒绝
  
TC-008: 数据清除测试
  输入: 脱敏完成后
  期望: 原始文件从内存和临时存储中完全清除
```

---

## 11. MoSCoW 优先级

### 11.1 Must (必须有)

| 功能 | 描述 | 验收条件 |
|------|------|----------|
| 文件解析 | 支持 PDF/Word/Excel/PPT/TXT 的解析 | 能正确提取文本内容和元数据 |
| 实体识别 | SLM + 规则双引擎识别 | Recall >= 99%, Precision >= 95% |
| 基础脱敏 | 遮蔽/替换脱敏模式 | 支持所有内置实体类型 |
| 加密沙箱 | AES-256 加密文件存储 | 文件不可被沙箱外软件读取 |
| 审计日志 | 记录所有关键操作 | 日志包含签名防篡改 |
| 自动化脱敏还原闭环 | 脱敏、审计、受控还原自动联动（P0） | 支持自动触发与闭环验证 |
| IPC API | 提供标准化接口 | Desktop/Desktop 服务端（Dify） 可正常调用 |
| 口令保护 | 用户口令验证 | 口令错误拒绝访问 |
| 行业模板 | 金融/医疗/法律预置模板 | 开箱即用 |

### 11.2 Should (应该有)

| 功能 | 描述 | 验收条件 |
|------|------|----------|
| FPE 可逆还原 | 格式保留加密还原 | 支持受控还原 |
| 元数据脱敏 | 自动清理文档元数据 | 覆盖主要元数据字段 |
| 代码解析 | AST 解析代码文件 | 支持主流编程语言 |
| 自定义规则 | 支持用户添加规则 | UI 可配置 |
| 策略导出 | 导出/导入策略配置 | JSON 格式 |
| 日志导出 | 导出审计日志 | CSV/JSON 格式 |

### 11.3 Could (可以有)

| 功能 | 描述 | 验收条件 |
|------|------|----------|
| 批量处理 | 批量文件脱敏 | 支持文件夹导入 |
| 规则调试 | 脱敏预览 | 可预览脱敏效果 |
| 多语言支持 | 英文实体识别 | 支持中英双语 |
| 插件扩展 | 第三方插件支持 | 预留扩展接口 |

### 11.4 Won't (暂不包含)

| 功能 | 说明 | 后续版本 |
|------|------|----------|
| 审批流 | 策略变更审批 | V1.1 企业版 |
| 反脱敏回填 | 自动回填 AI 结果 | V1.1 企业版 |
| DLP 提示 | 外发风险提醒 | V1.1 企业版 |
| 云端管理 | 云端策略控制 | V2.0 |
| 多租户 | 隔离架构 | V2.0 |
| 视频脱敏 | 视频文件处理 | V2.0 |

---

## 12. 实施计划

### 12.1 三阶段里程碑

```
                    CheersAI Vault V1.0 实施计划

    ┌──────────────────┐     ┌──────────────────┐     ┌──────────────────┐
    │   Phase 1: 基础  │     │   Phase 2: 核心  │     │   Phase 3: 企业  │
    │   2025.Q1        │     │   2025.Q2        │     │   2025.Q3        │
    ├──────────────────┤     ├──────────────────┤     ├──────────────────┤
    │ MVP 核心功能      │     │ 完善功能集       │     │ 企业级功能       │
    │                  │     │                  │     │                  │
    │ - 基础架构搭建    │     │ - FPE 可逆还原   │     │ - 审批流         │
    │ - 文件解析       │     │ - 策略管理       │     │ - DLP 风险提示   │
    │ - 实体识别       │     │ - 模板系统       │     │ - 反脱敏回填     │
    │ - 基础脱敏       │     │ - 审计增强       │     │ - 云端管理       │
    │ - 加密沙箱       │     │ - 代码解析       │     │                  │
    │ - 基础审计       │     │                  │     │                  │
    └──────────────────┘     └──────────────────┘     └──────────────────┘
             │                       │                       │
             ▼                       ▼                       ▼
        ┌─────────┐            ┌─────────┐            ┌─────────┐
        │ V0.5.0  │            │ V0.9.0  │            │ V1.1.0  │
        │ Alpha   │            │ Beta    │            │ GA      │
        └─────────┘            └─────────┘            └─────────┘
```

### 12.2 详细实施计划

#### Phase 1: 基础建设 (2025.Q1)

**目标**：完成 Vault 核心架构搭建，实现基础脱敏能力

**里程碑 1.1: 架构搭建** (第 1-4 周)

| 任务 | 负责人 | 交付物 |
|------|--------|--------|
| 项目初始化 | 侯傲宇 | 项目结构、CI/CD |
| IPC 通信层 | 侯傲宇 | IPC 通道框架 |
| 加密沙箱基础 | 侯傲宇 | 加密存储模块 |
| 密钥管理基础 | 侯傲宇 | 密钥管理模块 |

**里程碑 1.2: 解析引擎** (第 5-8 周)

| 任务 | 负责人 | 交付物 |
|------|--------|--------|
| PDF 解析器 | TBD | PDF 解析模块 |
| Word 解析器 | TBD | Word 解析模块 |
| Excel 解析器 | TBD | Excel 解析模块 |
| PPT 解析器 | TBD | PPT 解析模块 |

**里程碑 1.3: 识别与脱敏** (第 9-12 周)

| 任务 | 负责人 | 交付物 |
|------|--------|--------|
| SLM 引擎集成 | TBD | NER 模型推理 |
| 规则引擎 | TBD | 正则匹配模块 |
| 脱敏执行器 | TBD | 脱敏处理模块 |
| 基础审计 | TBD | 日志记录模块 |

**Phase 1 验收**：通过功能验收测试中的基础用例

#### Phase 2: 核心完善 (2025.Q2)

**目标**：完善 V1.0 功能集，支持企业场景

**里程碑 2.1: 可逆还原** (第 13-16 周)

| 任务 | 负责人 | 交付物 |
|------|--------|--------|
| FPE 算法实现 | TBD | FPE 加密模块 |
| 映射表管理 | TBD | 映射表 CRUD |
| 还原接口 | TBD | 还原 API |

**里程碑 2.2: 策略与模板** (第 17-20 周)

| 任务 | 负责人 | 交付物 |
|------|--------|--------|
| 策略配置系统 | TBD | 策略管理模块 |
| 行业模板库 | TBD | 金融/医疗/法律模板 |
| 模板 API | TBD | 模板接口 |

**里程碑 2.3: 企业增强** (第 21-24 周)

| 任务 | 负责人 | 交付物 |
|------|--------|--------|
| 代码 AST 解析 | TBD | 代码解析模块 |
| 审计导出 | TBD | 日志导出功能 |
| Desktop 集成 | TBD | 集成测试 |

**Phase 2 验收**：通过全部功能验收测试

#### Phase 3: 企业功能 (2025.Q3)

**目标**：完成 V1.1 企业版功能

**里程碑 3.1: 审批流** (第 25-28 周)

| 任务 | 负责人 | 交付物 |
|------|--------|--------|
| 审批流程引擎 | TBD | 审批模块 |
| 策略变更审批 | TBD | 审批 API |
| 通知机制 | TBD | 通知模块 |

**里程碑 3.2: 安全增强** (第 29-32 周)

| 任务 | 负责人 | 交付物 |
|------|--------|--------|
| DLP 风险检测 | TBD | DLP 模块 |
| 外发风险提示 | TBD | 风险提示 UI |
| 反脱敏回填 | TBD | 回填模块 |

**里程碑 3.3: 发版准备** (第 33-36 周)

| 任务 | 负责人 | 交付物 |
|------|--------|--------|
| 性能优化 | TBD | 性能报告 |
| 安全审计 | TBD | 安全审计报告 |
| 文档完善 | TBD | 用户/开发文档 |
| V1.1 GA 发布 | 全部 | 正式版本 |

### 12.3 发布计划

| 版本 | 目标日期 | 主要内容 |
|------|----------|----------|
| V0.5.0 (Alpha) | 2025-03-31 | 基础架构 + 核心解析 |
| V0.7.0 (Alpha) | 2025-05-31 | 实体识别 + 基础脱敏 |
| V0.9.0 (Beta) | 2025-07-31 | FPE + 模板 + 审计 |
| V1.0.0 (RC) | 2025-09-30 | 完整 V1.0 功能 |
| V1.0.0 (GA) | 2025-10-31 | 正式发布 |
| V1.1.0 (GA) | 2025-12-31 | 企业版功能 |

---

## 13. 风险与应对

### 13.1 技术风险

| 风险 | 影响 | 概率 | 应对措施 |
|------|------|------|----------|
| SLM 模型精度不达标 | 召回率/精确率低于目标 | 中 | 准备备选模型；规则引擎兜底；持续优化 |
| 大文件处理超时 | 用户体验差 | 中 | 分片处理；流式处理；进度反馈 |
| 加密性能瓶颈 | 处理速度慢 | 低 | 硬件加速；异步加密；缓存优化 |
| 内存占用过高 | 影响主机性能 | 中 | 内存池管理；及时释放；监控告警 |

### 13.2 安全风险

| 风险 | 影响 | 概率 | 应对措施 |
|------|------|------|----------|
| 密钥泄露 | 数据完全暴露 | 低 | 密钥分层；HSM 支持；密钥轮换 |
| 内存攻击 | 敏感数据窃取 | 低 | 内存加密；敏感数据即时清除；安全启动 |
| 审计日志篡改 | 合规风险 | 低 | 签名防篡改；WORM 存储；定期校验 |
| 权限绕过 | 未授权访问 | 低 | 安全编码审计；渗透测试；最小权限原则 |

### 13.3 合规风险

| 风险 | 影响 | 概率 | 应对措施 |
|------|------|------|----------|
| 法规更新 | 需要调整功能 | 中 | 持续跟踪法规变化；模块化设计便于更新 |
| 认证不通过 | 无法进入特定市场 | 中 | 提前与认证机构沟通；合规预审 |
| 跨境数据传输 | 合规要求差异 | 低 | 完全本地化设计；不跨境传输数据 |

### 13.4 项目风险

| 风险 | 影响 | 概率 | 应对措施 |
|------|------|------|----------|
| 资源不足 | 进度延期 | 中 | 阶段性交付；核心功能优先；外包支持 |
| 技术难度 | 某些功能无法实现 | 低 | 技术预研；POC 验证；范围调整 |
| 需求变更 | 返工 | 中 | 需求评审；变更管理；迭代计划 |

---

## 14. 附录

### 附录 A: 支持的文件格式列表

#### A.1 完全支持

| 格式 | 扩展名 | 解析完整性 | 备注 |
|------|--------|------------|------|
| PDF | .pdf | 100% | 文本层 + 图片 OCR |
| Word | .docx | 100% | OOXML 格式 |
| Word 旧版 | .doc | 95% | MS Word 97-2003 |
| Excel | .xlsx | 100% | 多 Sheet 支持 |
| Excel 旧版 | .xls | 90% | MS Excel 97-2003 |
| PowerPoint | .pptx | 100% | 文本框 + 形状 + 备注 |
| PowerPoint 旧版 | .ppt | 90% | MS PowerPoint 97-2003 |
| 文本 | .txt | 100% | UTF-8/GBK 自动识别 |
| CSV | .csv | 100% | 多编码支持 |
| JSON | .json | 100% | 任意 JSON 结构 |
| XML | .xml | 100% | 任意 XML 结构 |
| Markdown | .md | 100% | GFM 格式 |

#### A.2 代码文件支持

| 语言 | 扩展名 | AST 解析 | API Key 检测 |
|------|--------|----------|--------------|
| Python | .py | 是 | 是 |
| JavaScript | .js | 是 | 是 |
| TypeScript | .ts, .tsx | 是 | 是 |
| Java | .java | 是 | 是 |
| Go | .go | 是 | 是 |
| C/C++ | .c, .cpp, .h | 是 | 是 |
| C# | .cs | 是 | 是 |
| Ruby | .rb | 是 | 是 |
| PHP | .php | 是 | 是 |
| Rust | .rs | 是 | 是 |

#### A.3 计划支持 (V2.0)

| 格式 | 扩展名 | 预计支持版本 |
|------|--------|--------------|
| 图片 | .jpg, .png | V2.0 |
| 视频 | .mp4, .avi | V2.0 |
| 音频 | .mp3, .wav | V2.0 |
| 压缩包 | .zip, .rar | V2.0 |
| HTML | .html | V2.0 |
| RTF | .rtf | V2.0 |

### 附录 B: 支持的实体类型列表

#### B.1 个人身份信息

| 实体类型 | 标签 | 检测方式 | 风险等级 |
|----------|------|----------|----------|
| 身份证号 | id_card | 正则 | 高 |
| 护照号 | passport | 正则 | 高 |
| 驾驶证号 | driver_license | 正则 | 中 |
| 社保卡号 | social_security | 正则 | 高 |
| 银行卡号 | bank_card | 正则 + 校验位 | 高 |
| 信用卡号 | credit_card | Luhn 校验 | 高 |

#### B.2 联系信息

| 实体类型 | 标签 | 检测方式 | 风险等级 |
|----------|------|----------|----------|
| 手机号 | phone | 正则 | 中 |
| 固定电话 | telephone | 正则 | 低 |
| 电子邮箱 | email | 正则 | 中 |
| 地址 | address | NER + 规则 | 中 |

#### B.3 人员机构

| 实体类型 | 标签 | 检测方式 | 风险等级 |
|----------|------|----------|----------|
| 人名 | person_name | NER | 中 |
| 机构名 | org_name | NER | 中 |
| 地点 | location | NER | 低 |

#### B.4 金融信息

| 实体类型 | 标签 | 检测方式 | 风险等级 |
|----------|------|----------|----------|
| 金额 | money | NER + 规则 | 中 |
| 账号 | account_number | 正则 | 高 |
| IBAN | iban | 正则 | 高 |
| BIC/SWIFT | bic | 正则 | 高 |

#### B.5 医疗信息

| 实体类型 | 标签 | 检测方式 | 风险等级 |
|----------|------|----------|----------|
| 病历号 | medical_record_number | 正则 | 高 |
| 医保卡号 | insurance_number | 正则 | 高 |
| 诊断结果 | diagnosis | NER | 中 |
| 药品名称 | medication | NER | 低 |

#### B.6 法律信息

| 实体类型 | 标签 | 检测方式 | 风险等级 |
|----------|------|----------|----------|
| 案件号 | case_number | 规则 | 高 |
| 合同编号 | contract_number | 规则 | 中 |
| 法院名称 | court_name | NER | 中 |

#### B.7 技术信息

| 实体类型 | 标签 | 检测方式 | 风险等级 |
|----------|------|----------|----------|
| API Key | api_key | 正则 | 极高 |
| 私有 IP | private_ip | 正则 | 低 |
| 内网域名 | internal_domain | 正则 | 低 |
| 私有函数名 | private_function | AST | 中 |
| 私有变量名 | private_variable | AST | 中 |
| 数据库连接串 | db_connection | 规则 | 高 |
| 密码/密钥 | secret | AST + 规则 | 极高 |

### 附录 C: 脱敏策略对照表

#### C.1 脱敏模式速查

| 模式 | 输入示例 | 输出示例 | 适用实体 |
|------|----------|----------|----------|
| mask | 310101199001011234 | 310***********1234 | id_card, phone |
| replace | 张三 | 客户A | person_name |
| encrypt | 6222021234567890123 | enc_a1b2c3d4e5f6 | bank_card (可逆) |
| delete | [全部内容] | (空) | 高风险且无需保留 |
| partial | 北京市朝阳区XX路1号 | 北京市朝阳区 | address |
| sequential | 张三, 李四 | 客户1, 客户2 | person_name |

#### C.2 风险等级与脱敏建议

| 风险等级 | 示例实体 | 推荐模式 | 是否可逆 | 审计要求 |
|----------|----------|----------|----------|----------|
| 极高 | api_key, password | delete | 否 | 是 |
| 高 | id_card, bank_card | encrypt/mask | 否/受控 | 是 |
| 中 | phone, email, person_name | replace/mask | 可选 | 是 |
| 低 | private_ip, org_name | replace | 否 | 否 |

#### C.3 行业合规对照

| 行业 | 法规要求 | 必须脱敏的实体 | 特殊要求 |
|------|----------|----------------|----------|
| 银行/金融 | 等保2.0, PCI-DSS | 身份证, 银行卡, 账号 | 不可逆, 审计完整 |
| 医疗 | HIPAA, 等保2.0 | 病历号, 医保卡, 患者信息 | PHI 全覆盖 |
| 保险 | 等保2.0 | 身份证, 保单号, 受益人 | 不可逆 |
| 法律 | 等保2.0, 律师法 | 当事人信息, 案件信息 | 案件号保留 |
| 政府 | 等保2.0, 数据安全法 | 敏感政府信息 | 最高安全级别 |

---

## 文档变更记录

| 版本 | 日期 | 修改人 | 变更内容 |
|------|------|--------|----------|
| V1.0 | 2026-03-21 | 西北人 | 初稿创建 |
| V1.3 | 2026-03-22 | 西北人 | CS架构适配；企业版内网存储支持；映射文件加密防泄露强化 |
| V1.2 | 2026-03-22 | 西北人 | 产品矩阵更新（Studio合并入Desktop）；删除Studio独立产品引用；增加FileBay(Gitea)集成说明 |
| V1.1 | 2026-03-22 | 西北人 | 产品矩阵更新、企业级能力增强、映射表安全设计、FileBay 集成、竞品分析与优先级调整 |

---

*本文档版权归 CheersAI 所有，仅供内部使用*
