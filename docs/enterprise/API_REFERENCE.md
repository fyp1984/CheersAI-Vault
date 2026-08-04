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
  只返回脱敏后的 Markdown。

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
`passport`、`use_sensitive_terms`（启用已配置的敏感词库，无需逐词列出）。

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
Content-Disposition: attachment; filename="masked-a1....md"
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
