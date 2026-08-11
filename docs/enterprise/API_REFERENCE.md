# CheersAI Vault Pro Runtime — 客户测试 API 参考（首版，4 项）

> **适用范围**：本文档仅描述 Linux 内网客户测试部署中 `vault-runtime-api`
> 对外提供的 **四项** HTTP API：批量提交、进度轮询（含失败信息）、结果下载、
> 健康检查。这是老师针对本次客户测试明确圈定的首版范围（见
> `.codex-local/规则/决策说明.md`），不是 Runtime 全部已实现的接口。
>
> Runtime 实际还实现了 `GET /api/v1/batches`（列表）、
> `POST /api/v1/files/{file_id}/retry`（重试）、
> `POST /api/v1/artifacts/{artifact_id}/restore`（服务器内部映射恢复）、
> `/api/v1/rules`、`/api/v1/ocr/status`、`/api/v1/previews/**`、
> `/api/v1/sensitive-terms/**`、`/api/v1/operation-logs/**` 等接口——这些接口
> 存在且被原 Vault 浏览器前端使用，但**不在本次客户测试的承诺范围内**，本文档
> 不对它们的字段、稳定性或行为作任何保证，客户内网系统不应依赖本文档之外的
> 接口。

## 0. 安全边界（必读）

- **本版本没有应用层鉴权** —— 没有 API Key、没有用户登录、没有会话、没有
  权限体系。唯一的网络边界是 Nginx 的客户 CIDR 白名单
  （见 [`deploy/linux/nginx-cheersai-vault.conf`](../../deploy/linux/nginx-cheersai-vault.conf)）。
  **任何能够访问 Nginx 监听地址的主机都能调用这四项 API。**
  这是一个隔离测试内网的临时安排，**不是正式生产安全方案**，不得将本部署
  暴露到公网或不受信任的网络。
- CORS（`VAULT_RUNTIME_CORS_ORIGINS`）只控制浏览器同源策略，**不是接口鉴权**，
  服务器到服务器的调用（如客户内网系统直接调用 API）完全不受 CORS 限制。
- 错误响应经过消毒：不包含脱敏前原文、SQL、服务器文件路径、堆栈或内部命令
  （见 §5）。
- 不提供 `.cmap`（映射文件）下载接口；`GET /api/v1/artifacts/{artifact_id}`
  只返回脱敏后的 Markdown，且下载文件名会沿用已脱敏的展示名。

## 1. 基础信息

| 项目 | 值 |
|---|---|
| Base URL（客户内网系统经 Nginx 调用） | `http://<Nginx 内网地址>/api/v1` |
| Base URL（本机直连 Runtime，仅用于本机调试） | `http://127.0.0.1:8787/api/v1` |
| 认证 | 无（见 §0） |
| 请求/响应格式 | JSON（除文件上传用 `multipart/form-data`、下载返回 `text/markdown`） |
| 幂等键 / 取消 / 多租户 | **不提供**（见 §6 非目标） |

Runtime 端口默认 `8787`，只监听 `127.0.0.1`；客户内网系统**必须**通过 Nginx
反向代理访问，不能直连 `8787`（该端口未对外网络暴露）。

## 2. `POST /api/v1/batches` — 批量提交

提交一个或多个文件进行脱敏处理，Runtime 立即返回批次和文件 ID，实际处理
在后台异步进行（需轮询 §3 获取结果）。

**请求**：`multipart/form-data`，字段：

| 字段名 | 是否可重复 | 说明 |
|---|---|---|
| `files` | 是（每个文件一个 part） | 待脱敏文件的原始字节，`filename` 用于生成安全展示名 |
| `rule_ids` | 否 | JSON 字符串数组（如 `["id_card","phone","email"]`），也接受逗号分隔字符串作为兼容格式；至少一个受支持规则 ID |

支持的 `rule_ids` 值：`id_card`、`phone`、`email`、`bank_card`、`ipv4`、
`passport`、`chinese_name`、`use_sensitive_terms`（启用已配置的敏感词库，无需逐词列出）。

说明：

- `filename` 不仅用于安全展示名生成；当文件名中命中启用的脱敏规则时，Runtime 会在入库前同步对文件名做脱敏处理。
- 因此后续 `batch/files[].display_name`、预览文件名、下载文件名都会使用脱敏后的展示名，而不是原始上传文件名。

```bash
curl -sS -X POST "http://<Nginx地址>/api/v1/batches" \
  -F "files=@/path/to/report.docx" \
  -F "files=@/path/to/notes.txt" \
  -F 'rule_ids=["id_card","phone","email"]'
```

**成功响应**：`202 Accepted`

```json
{
  "batch_id": "b3b2e6b6-....",
  "files": [
    { "file_id": "f1...", "display_name": "report.docx" },
    { "file_id": "f2...", "display_name": "notes.txt" }
  ]
}
```

**已知错误码**（响应体见 §5 结构）：

| HTTP | code | 触发条件 |
|---|---|---|
| 400 | `INVALID_MULTIPART` | multipart 请求体格式错误或某个字段无法读取 |
| 400 | `UNEXPECTED_FIELD` | 出现 `files`/`rule_ids`/`rules`/`restore_mode` 之外的字段 |
| 400 | `FILES_REQUIRED` | 没有提交任何文件 |
| 400 | `INVALID_RULES` | `rule_ids` 缺失、为空或包含不受支持的规则 ID |
| 400 | `INPUT_FORMAT_UNSUPPORTED` | 某个文件的扩展名/格式不受支持 |
| 413 | `INPUT_LIMIT_EXCEEDED` | 单文件超过大小限制、批次文件数超限，或批次总字节数超限 |

## 3. `GET /api/v1/batches/{batch_id}` — 进度查询与失败信息

轮询一个批次的整体状态和每个文件的状态，直到批次到达终态。

```bash
curl -sS "http://<Nginx地址>/api/v1/batches/b3b2e6b6-...."
```

**成功响应**：`200 OK`

```json
{
  "batch": {
    "batch_id": "b3b2e6b6-....",
    "status": "CompletedWithErrors",
    "file_count": 2,
    "completed_count": 1,
    "failed_count": 1,
    "masked_entity_count": 5,
    "created_at": "2026-07-31T02:00:00Z",
    "updated_at": "2026-07-31T02:00:03Z"
  },
  "files": [
    {
      "file_id": "f1...",
      "display_name": "report.docx",
      "input_format": "docx",
      "status": "Completed",
      "attempt": 1,
      "masked_entity_count": 5,
      "artifact_id": "a1...",
      "error_code": null,
      "error_message": null,
      "restore_available": true
    },
    {
      "file_id": "f2...",
      "display_name": "notes.txt",
      "input_format": "txt",
      "status": "Failed",
      "attempt": 1,
      "masked_entity_count": null,
      "artifact_id": null,
      "error_code": "INPUT_CORRUPTED",
      "error_message": "Input could not be decoded as valid text",
      "restore_available": false
    }
  ]
}
```

`batch.status` 取值（终态：`Completed`、`CompletedWithErrors`、`Failed`；
`Running` 为非终态，需继续轮询）：

| 值 | 含义 |
|---|---|
| `Running` | 仍有文件在队列中或处理中 |
| `Completed` | 全部文件成功完成（终态） |
| `CompletedWithErrors` | 至少一个成功、至少一个失败（终态） |
| `Failed` | 全部文件失败（终态） |

`files[].status` 取值：`Pending`、`Processing`、`Completed`、`Failed`。

**轮询建议**：客户内网系统应轮询直到 `batch.status` 为
`Completed`/`CompletedWithErrors`/`Failed` 三者之一再读取 `files[]`；间隔建议
不低于 1 秒，本版本未提供 webhook/推送通知。

**失败信息**：只在 `files[].status == "Failed"` 时出现 `error_code` /
`error_message`，二者均已消毒，**不包含脱敏前原文、文件内容片段、SQL、服务器
路径或堆栈**。已观察到的文件级错误码包括（不保证穷尽，新增内部失败类型不
另行通知，客户系统应把未识别的 `error_code` 当作不可自动分类的失败处理）：

`INPUT_READ_FAILED`、`INPUT_FORMAT_UNSUPPORTED`、`INPUT_CORRUPTED`、
`INPUT_ENCRYPTED`、`INPUT_NO_CONTENT`、`OCR_COMPONENT_REQUIRED`、
`MASKING_FAILED`、`MAPPING_ENCODE_FAILED`、`OUTPUT_WRITE_FAILED`。

**已知错误码（请求本身）**：

| HTTP | code | 触发条件 |
|---|---|---|
| 404 | `NOT_FOUND` | `batch_id` 不存在 |

## 4. `GET /api/v1/artifacts/{artifact_id}` — 结果下载

下载单个已完成文件的脱敏结果（Markdown 格式）。`artifact_id` 来自 §3 响应中
对应文件的 `artifact_id` 字段。

```bash
curl -sS -o masked-report.md \
  "http://<Nginx地址>/api/v1/artifacts/a1...."
```

**成功响应**：`200 OK`

```
Content-Type: text/markdown; charset=utf-8
Content-Disposition: attachment; filename="姓名1-PHONE2_脱敏.md"
```

响应体是脱敏后的 Markdown 纯文本。

**限制**：

- 只能下载**已完成**（`FileStatus::Completed`）文件对应的 artifact；未完成、
  失败或不存在的 `artifact_id` 一律返回 404，不区分具体原因（避免探测批次
  内部状态）。
- **不提供 `.cmap`（映射文件）下载接口** —— 映射数据只用于 Runtime 内部的
  服务器端恢复功能（`POST /api/v1/artifacts/{artifact_id}/restore`，不在本文档
  承诺范围内），客户内网系统无法通过任何公开接口取得原始映射。
- 不提供批量打包下载（如 zip）；需要对 §3 响应中每个成功文件的
  `artifact_id` 分别调用本接口。

**已知错误码**：

| HTTP | code | 触发条件 |
|---|---|---|
| 404 | `NOT_FOUND` | `artifact_id` 不存在，或对应文件尚未完成 |

## 5. `GET /api/v1/health` — 部署就绪检查

```bash
curl -sS "http://<Nginx地址>/api/v1/health"
```

**成功响应**：`200 OK`

```json
{ "status": "ready", "version": "0.1.0" }
```

仅用于确认 Runtime 进程存活并已完成启动初始化（数据目录打开、旧任务恢复等），
**不代表** OCR、LibreOffice 等可选组件已就绪 —— 那些状态属于本文档范围外的
`/api/v1/ocr/status` 接口。健康检查不需要任何请求体或参数，可用于 systemd
探活或负载均衡健康检查（本版本不含负载均衡部署）。

## 6. 通用错误响应结构

四项 API 出错时统一返回该结构（除 `GET /api/v1/health` 本身不会失败）：

```json
{
  "code": "INVALID_RULES",
  "message": "At least one supported rule ID is required",
  "retryable": false
}
```

| 字段 | 说明 |
|---|---|
| `code` | 机器可读错误码，见各接口章节的错误码表 |
| `message` | 面向人类的简短说明，**已消毒**，不含原文/路径/堆栈 |
| `retryable` | `true` 表示同一请求原样重试可能成功（如底层存储瞬时故障）；`false` 表示需要先修正请求或状态 |

请求本身格式错误但未命中上述具体错误码时，返回：

```json
{ "code": "INVALID_REQUEST", "message": "The request could not be processed", "retryable": false }
```

## 7. 成功 / 部分失败 / 全部失败 —— 典型闭环示例

```bash
BASE="http://<Nginx地址>/api/v1"

# 1) 提交
RESP=$(curl -sS -X POST "$BASE/batches" \
  -F "files=@good.txt" -F "files=@bad.bin" \
  -F 'rule_ids=["phone","email"]')
BATCH_ID=$(echo "$RESP" | jq -r .batch_id)

# 2) 轮询直到终态
while true; do
  DETAIL=$(curl -sS "$BASE/batches/$BATCH_ID")
  STATUS=$(echo "$DETAIL" | jq -r .batch.status)
  case "$STATUS" in
    Running) sleep 1 ;;
    Completed|CompletedWithErrors|Failed) break ;;
  esac
done

# 3) 读取每个文件的结果 / 失败信息
echo "$DETAIL" | jq -c '.files[]'

# 4) 下载所有成功文件
echo "$DETAIL" | jq -r '.files[] | select(.artifact_id != null) | .artifact_id' \
  | while read -r AID; do
      curl -sS -o "masked-$AID.md" "$BASE/artifacts/$AID"
    done
```

`CompletedWithErrors` 时，`good.txt` 会有 `artifact_id` 可下载，`bad.bin`
只会有 `error_code`/`error_message`（如 `INPUT_FORMAT_UNSUPPORTED`），没有
`artifact_id`，下载接口对它返回 404。

## 8. 非目标（本版本明确不提供）

- 不提供 SDK 或客户端库；只能自行用 HTTP 客户端调用上述接口。
- 不提供稳定 OpenAPI/Swagger 规范；本文档是唯一的接口说明来源，字段以
  Runtime 实际代码（`apps/vault-runtime-api/src/lib.rs`、
  `src-tauri/crates/service-contracts/src/lib.rs`）为准，两者不一致时以代码为准。
- 不提供幂等键；重复提交 `POST /api/v1/batches` 会创建新的独立批次。
- 不提供取消接口；已提交批次无法中途取消。
- 不提供权限/用户体系；见 §0。
- 不提供多租户隔离；同一 Runtime 实例的所有调用方共享同一个批次/文件
  命名空间和数据目录。
- 不提供 Webhook / 服务器推送；结果只能通过轮询 §3 获取。

## 9. 企业版 API 接入全流程

本节面向企业内网系统的技术接入人员，给出一条从申请、环境检查、联调到上线的
最小闭环路径。**请先接受一个前提**：当前版本对外正式承诺的只有 §2-§5 的四项
API；用户、权限、OAuth2.0、API Key、知识库检索与权限映射都不在 Runtime 原生
能力范围内，若企业必须启用这些能力，应在 Runtime 外围增加 API Gateway 或中间层。

### 9.1 前置准备清单

| 类别 | 需要准备的内容 | 责任方 |
|---|---|---|
| 网络开通 | Nginx 内网访问地址、调用方源 IP/CIDR、测试网段与生产网段 | 企业网络管理员 |
| 环境信息 | 部署形态（Linux + Nginx + Runtime，或本地 Docker 验证）、目标域名/IP、端口规划、TLS 终止位置 | 部署管理员 |
| 数据样本 | 至少 3 组脱敏联调用测试文件，覆盖纯文本、Office/PDF、异常样本；不得使用真实生产敏感数据 | 对接系统负责人 |
| 规则确认 | 需要启用的 `rule_ids`，是否使用共享敏感词库，是否需要 OCR、FileBay | 业务负责人 + 部署管理员 |
| 资质与审批 | 企业内部变更单、网络白名单审批、数据分类分级审批、测试数据合规确认 | 企业内控 / 安全 / 运维 |
| 运维交接 | 值班联系人、故障升级群、上线窗口、回滚负责人 | 交付负责人 |

### 9.2 接入步骤

1. **申请访问权限**：由企业内控或运维团队提交内网访问申请，明确调用源网段、
   联调时间窗、是否需要 OCR 与 FileBay。当前版本没有系统内建审批流，所有“申请”
   和“审核”均指企业侧人工流程。
2. **提交环境与资质材料**：至少提供调用源 IP/CIDR、联调联系人、测试数据说明、
   脱敏规则清单、是否需要扫描件 OCR、是否需要 FileBay 上传。
3. **完成网络与环境校验**：
   - 浏览器或 `curl` 能访问 `GET /api/v1/health`；
   - Nginx 已正确反代 `/api/`；
   - Runtime 数据目录、OCR、LibreOffice、FileBay（如启用）已按部署文档配置。
4. **执行接口烟雾验证**：先用一份虚构文本文件调用 `POST /api/v1/batches`，
   再轮询 `GET /api/v1/batches/{batch_id}`，确认能获得 `artifact_id` 并下载结果。
5. **开展场景联调**：对接系统按照 §11 的场景示例完成批量提交、轮询、下载、
   敏感词库同步或外围知识库入库等动作，记录请求样例、错误码和重试结果。
6. **上线前复核**：确认监控、日志、回滚与白名单配置已到位，再切换到生产网段。

### 9.3 环境校验命令

```bash
BASE="http://<Nginx内网地址>/api/v1"

# 1) 部署就绪
curl -sS "$BASE/health"

# 2) 提交虚构样本
curl -sS -X POST "$BASE/batches" \
  -F "files=@./samples/demo.txt" \
  -F 'rule_ids=["phone","email"]'
```

若 `GET /api/v1/health` 失败，不应继续联调业务接口，应先回到部署侧排查 Nginx、
Runtime 和白名单。

## 10. 身份认证与访问控制配置

### 10.1 当前版本真实边界

| 能力 | 当前状态 | 说明 |
|---|---|---|
| OAuth2.0 授权流程 | 不支持 | Runtime 不处理授权码、令牌交换、刷新令牌、用户会话 |
| API Key | 不支持 | Runtime 不读取 `Authorization`、`x-api-key` 等应用层凭据 |
| 应用层用户身份 | 不支持 | 没有登录、账号、RBAC、多租户或权限树 |
| 权限映射接口 | 不支持 | Runtime 不保存“外部用户/角色 -> 文件/批次”映射 |
| 当前唯一访问边界 | 已支持 | 仅有 Nginx CIDR 白名单与部署侧操作系统权限 |

### 10.2 推荐落地方式

若企业接入规范要求 OAuth2.0、API Key、租户隔离或审计签名，**应在 Runtime 外侧
增加 API Gateway / 反向代理 / 中间层**，由外围组件承担认证和授权，Runtime 仍只
负责脱敏处理本身。

```text
企业应用 / 用户
      ↓
企业 SSO / OAuth2.0 / API Gateway
      ↓  (完成认证、鉴权、限流、审计)
CheersAI Vault Runtime 4 项正式 API
```

推荐做法：

- Gateway 完成 OAuth2.0 或 API Key 校验，并把合法请求转发给 Runtime。
- Gateway 承担限流、审计、来源 IP 约束、调用日志落库。
- Gateway 维护外部用户到内部业务记录的权限关系；Runtime 不感知用户身份。
- Gateway 或企业中间层维护“源文档 ID / 外部用户 / 批次 ID / artifact_id”的
  关联表，供后续查询、权限控制和审计使用。

### 10.3 不建议的做法

- 不要把 Runtime 直接暴露到公网。
- 不要把 `CORS` 当作接口鉴权。
- 不要假设 Runtime 能识别用户令牌、角色或部门信息。
- 不要在企业代码中依赖本文档未承诺的内部接口稳定性。

## 11. 场景化接口调用示例

### 11.1 知识库源文档同步到脱敏链路

**当前版本没有“知识库数据同步 API”**。若企业现有知识库系统希望把文档接入
CheersAI Vault，应由企业侧先导出文件，再调用正式承诺的 `POST /api/v1/batches`
进行脱敏，随后下载脱敏后的 Markdown 结果，最后由企业侧写回自己的知识库或检索
系统。

最小链路如下：

```text
企业知识库导出原始文件
      ↓
POST /api/v1/batches
      ↓
GET /api/v1/batches/{batch_id}
      ↓
GET /api/v1/artifacts/{artifact_id}
      ↓
企业侧写入自有知识库 / 搜索引擎
```

Python 示例：

```python
import json
import time
import requests

BASE = "http://<Nginx内网地址>/api/v1"

with open("contract.docx", "rb") as f:
    resp = requests.post(
        f"{BASE}/batches",
        files={"files": ("contract.docx", f)},
        data={"rule_ids": json.dumps(["phone", "email", "use_sensitive_terms"])},
        timeout=60,
    )
resp.raise_for_status()
batch_id = resp.json()["batch_id"]

while True:
    detail = requests.get(f"{BASE}/batches/{batch_id}", timeout=30)
    detail.raise_for_status()
    payload = detail.json()
    status = payload["batch"]["status"]
    if status in {"Completed", "CompletedWithErrors", "Failed"}:
        break
    time.sleep(1)

for item in payload["files"]:
    artifact_id = item.get("artifact_id")
    if not artifact_id:
        continue
    download = requests.get(f"{BASE}/artifacts/{artifact_id}", timeout=60)
    download.raise_for_status()
    with open(f"masked-{artifact_id}.md", "wb") as out:
        out.write(download.content)
```

集成建议：

- 以企业侧原始文档 ID 作为本地业务主键，保存 `batch_id` 与 `artifact_id`。
- 对 `CompletedWithErrors` 做逐文件处理，不要按“整批成功”假设直接入库。
- 下载后的 Markdown 应由企业侧完成索引、归档和访问控制。

### 11.2 敏感词库数据同步

敏感词库相关接口已由浏览器前端使用，但**不在首版客户 API 的正式承诺范围内**。
若企业决定调用，应视为“受控内部接口”，升级时自行复核兼容性。

支持的内部接口如下：

| 接口 | 方法 | 用途 |
|---|---|---|
| `/api/v1/sensitive-terms` | `GET` | 按分类/关键字列出敏感词 |
| `/api/v1/sensitive-terms` | `POST` | 新建敏感词 |
| `/api/v1/sensitive-terms/{id}` | `PUT` | 更新敏感词 |
| `/api/v1/sensitive-terms/{id}` | `DELETE` | 删除敏感词 |
| `/api/v1/sensitive-terms/categories` | `GET` | 获取分类列表 |
| `/api/v1/sensitive-terms/stats` | `GET` | 获取统计信息 |
| `/api/v1/sensitive-terms/import` | `POST` | CSV 导入 |
| `/api/v1/sensitive-terms/export` | `GET` | CSV 导出 |

列表示例：

```bash
curl -sS \
  "http://<Nginx内网地址>/api/v1/sensitive-terms?category=合同&query=保密&enabled_only=true"
```

返回示例：

```json
{
  "terms": [
    {
      "id": "term-001",
      "term": "保密项目A",
      "category": "合同",
      "description": "客户项目代号",
      "enabled": true,
      "created_at": "2026-08-09T06:00:00Z",
      "updated_at": "2026-08-09T06:00:00Z"
    }
  ]
}
```

新建示例：

```bash
curl -sS -X POST "http://<Nginx内网地址>/api/v1/sensitive-terms" \
  -H "Content-Type: application/json" \
  -d '{
    "term": "保密项目A",
    "category": "合同",
    "description": "客户项目代号"
  }'
```

导入示例：

```bash
curl -sS -X POST "http://<Nginx内网地址>/api/v1/sensitive-terms/import" \
  -F "file=@./sensitive_terms.csv"
```

导入成功响应：

```json
{ "imported_count": 128 }
```

CSV 约束：

- 文件必须是 UTF-8 编码，可带 BOM。
- 表头必须精确为：`分类,敏感词,描述,状态`。
- `状态` 只能是 `启用` 或 `禁用`。
- 文件大小上限 `5 MB`，最大 `10,000` 行。

常见错误码：

| HTTP | code | 说明 |
|---|---|---|
| 400 | `SENSITIVE_TERM_INVALID` | 单条敏感词字段非法 |
| 409 | `SENSITIVE_TERM_DUPLICATE` | 敏感词重复 |
| 404 | `SENSITIVE_TERM_NOT_FOUND` | 指定 ID 不存在 |
| 400 | `SENSITIVE_TERMS_IMPORT_INVALID` | CSV 结构、表头、编码或内容非法 |
| 413 | `INPUT_LIMIT_EXCEEDED` | CSV 文件大小或行数超过限制 |

### 11.3 内容检索能力说明

**当前版本没有全文检索 API，也没有“按关键词搜索脱敏内容”的服务端接口。**
可用能力只有：

- `GET /api/v1/batches/{batch_id}`：查询批次和文件处理状态；
- `GET /api/v1/artifacts/{artifact_id}`：下载单个脱敏 Markdown。

因此，企业若要实现知识库检索，应采用以下方式：

1. 企业侧完成文档上传与脱敏；
2. 下载脱敏 Markdown；
3. 在企业自有知识库、向量库或搜索引擎中建立索引；
4. 搜索请求仍走企业自有检索系统，而不是 Runtime。

### 11.4 权限映射能力说明

**当前版本没有权限映射接口。** Runtime 不认识用户、角色、部门、租户，也不会
校验“谁可以查看哪个 artifact”。建议企业在自有系统中维护如下关联信息：

```json
{
  "external_doc_id": "kb-2026-001",
  "batch_id": "b3b2e6b6-....",
  "artifact_id": "a1....",
  "owner_user_id": "u-1001",
  "owner_department": "legal",
  "visibility_scope": ["legal", "audit"]
}
```

该映射应由企业侧网关、知识库或审计系统维护；Runtime 仅返回 `batch_id`、
`artifact_id` 和处理状态，不承担权限决策。

## 12. 联调测试方法与环境要求

### 12.1 联调环境要求

| 项目 | 要求 |
|---|---|
| 网络 | 仅允许企业内网访问，Nginx 白名单已包含调用源网段 |
| 数据 | 使用虚构或脱敏后的测试文件，禁止直接使用真实生产数据 |
| 路径 | 优先经 Nginx 联调，不建议长期直连 `127.0.0.1:8787` |
| 依赖 | OCR、LibreOffice、FileBay 若属于本次范围，须提前安装并单独校验 |
| 隔离 | 测试环境与生产环境分开，测试网段、环境变量、数据目录独立 |

### 12.2 建议联调用例

1. **基础可用性**：`GET /api/v1/health` 返回 `200`。
2. **正常提交**：TXT、DOCX、PDF 至少各提交一份，确认能拿到 `artifact_id`。
3. **异常样本**：提交超限文件、损坏文件、加密 PDF，确认错误码符合预期。
4. **扫描件场景**：提交图片型 PDF，确认 OCR 已启用时可完成；未启用时返回
   `OCR_COMPONENT_REQUIRED`。
5. **重启恢复**：Runtime 重启后再次查询既有 `batch_id`，确认批次和产物未丢失。
6. **可选链路**：若启用了 FileBay 或敏感词库同步，额外验证其成功与失败分支。

### 12.3 联调通过标准

- 正常请求能在约定时间内返回，且响应结构与本文档一致。
- 异常请求返回受控错误码，不泄露原文、路径、SQL 或堆栈。
- `CompletedWithErrors`、`Failed` 两类场景都能被企业侧正确识别和处理。
- Runtime 重启后既有批次和产物仍可读取。
- 部署、业务、运维三方都已确认回滚路径。

## 13. 接入后的稳定性监控方案

建议把监控分成四层：

### 13.1 存活与网络

- 监控 `GET /api/v1/health` 的可达性、HTTP 状态码和响应时间。
- 监控 Nginx 4xx/5xx 比例，重点关注白名单误拦截与上游连接失败。
- 监控 Runtime 进程存活、重启次数和监听端口状态。

### 13.2 业务成功率

- 统计 `POST /api/v1/batches` 的提交量、失败量、平均文件数。
- 统计 `batch.status` 中 `Completed`、`CompletedWithErrors`、`Failed` 的占比。
- 汇总高频 `error_code`，如 `INPUT_FORMAT_UNSUPPORTED`、`OCR_TIMEOUT`、
  `OCR_COMPONENT_REQUIRED`、`INPUT_LIMIT_EXCEEDED`。

### 13.3 依赖组件

- OCR：定期检查 `/api/v1/ocr/status`，关注 `status` 是否为 `ready`。
- FileBay：若启用，检查 `/api/v1/filebay/status` 是否为 `configured`，并对
  `FILEBAY_*` 错误码设置告警。
- 存储：监控 `VAULT_RUNTIME_DATA_DIR` 所在磁盘容量、inode、读写错误。

### 13.4 运维告警建议

- `health` 连续失败。
- Nginx 5xx 或 upstream connect error 持续升高。
- 单批次长时间停留在 `Running`。
- OCR / FileBay 状态从可用变为不可用。
- 数据目录磁盘空间低于企业自定阈值。

建议同时保留：

- Nginx access/error 日志；
- systemd 或进程日志；
- 企业网关审计日志；
- 企业侧维护的 `external_doc_id` / `batch_id` / `artifact_id` 关联表。
