# Runtime API 功能测试报告（17 项断言摘要）

> 工程内交付版，整理自内部验收任务
> `TASK-RUNTIME-API-FUNCTIONAL-PERFORMANCE-ACCEPTANCE-001` 第二轮（R2）已通过
> Review 的功能结果。测试方案（范围、环境与安全边界、判定标准）见
> [`TEST_PLAN.md`](./TEST_PLAN.md)；机器可读数据见
> [`data/functional-results.json`](./data/functional-results.json)（与本报告
> 17 项逐字对应）。

## 1. 结论速览

| 结论 | 数量 |
|---|---:|
| `PASS` | 17 |
| `FAIL` | 0 |
| `BLOCKED` | 0 |
| `NOT_RUN` | 0 |

17 项黑盒功能断言在产品基线 HEAD `622b8d73ccf8000f446ae4a440905f898a69ba07`
上全部通过，覆盖 [`API_REFERENCE`](../../enterprise/API_REFERENCE.md) 承诺的
四项 API，另含一项被该文档标注为"存在但不在首版承诺范围"的服务器映射恢复
接口。

## 2. 执行环境

| 项目 | 值 |
|---|---|
| 产品基线 HEAD | `622b8d73ccf8000f446ae4a440905f898a69ba07` |
| 被测 Runtime 二进制 SHA-256 | `4598762941a761f54216dd477f72ce30eec24a3ebd4f54eeb5847a4b3f220909`（见 [`data/machine-info.json`](./data/machine-info.json)） |
| 测试机器 | macOS（Darwin 25.6.0，arm64，Apple M2，8 逻辑核，16 GiB 内存） |
| Runtime 监听 | 仅 `127.0.0.1`；本轮使用随机选定端口的隔离测试实例，记录端口 18787（客户部署默认端口 8787 见 [`API_REFERENCE`](../../enterprise/API_REFERENCE.md)） |
| 数据目录 | 每轮随机隔离的临时目录；结束后精确停止进程 PID、确认端口释放并删除临时目录 |
| 测试输入 | 全部为虚构 UTF-8 TXT 文件与随机 UUID；不读取用户文件，不配置 FileBay，不使用 Token、PIN、Keychain 或真实口令 |

## 3. 数据口径声明（必读）

**本报告与 [`data/functional-results.json`](./data/functional-results.json)
是断言摘要，不是完整 HTTP 报文存档。**

1. 原始请求/响应正文从未保留，无法据此重建任何一条 HTTP 报文。
2. 存档字段仅包括：用例标识（`case`）、HTTP 状态、错误码（`error_code`）、
   安全错误结构标志（`safe_error_shape`）、响应字节数
   （`response_bytes`）、响应 SHA-256（`response_sha256`）、断言结论
   （`result`），以及少数用例的少量附加断言字段（`content_type`、
   `restored_count_header`、终端状态摘要字符串 `detail`）。
3. `response_sha256` 只用于一致性核对（例如重启前后下载同一产物），不代表
   保留了响应内容。
4. 测试输入为虚构内容，但原文与恢复明文同样未存档；本目录不含凭据、真实
   敏感数据、恢复明文或本机绝对路径。

## 4. 17 项功能断言明细

"预期"列与 [`data/functional-results.json`](./data/functional-results.json)
的 `expected` 字段逐字一致；"实际"列摘自该文件的对应字段；SHA-256 缩写为
前 8 位，完整值见该 JSON。

| # | case | 方法 | 路径 | 输入 | 预期 | 实际 | 结论 |
|---|---|---|---|---|---|---|---|
| 1 | `health` | GET | `/api/v1/health` | 无请求体（部署就绪检查） | `200 ready` | HTTP 200，响应 36 B | PASS |
| 2 | `single_submit_poll` | POST 后 GET 轮询 | `/api/v1/batches` → `/api/v1/batches/{batch_id}` | 单个虚构 TXT（1 文件、1 规则） | `202 then Completed` | HTTP 202（提交响应 140 B），轮询至终态 `Completed` | PASS |
| 3 | `artifact_download` | GET | `/api/v1/artifacts/{artifact_id}` | 上一项产生的脱敏产物 | `200 masked Markdown` | HTTP 200，`content_type=text/markdown; charset=utf-8`，响应 10233 B（SHA-256 `5b09fe0c…`） | PASS |
| 4 | `artifact_restore` | POST | `/api/v1/artifacts/{artifact_id}/restore`（存在但不在首版承诺范围） | 上一项产物，服务器内部映射恢复 | `200 exact fictional values restored` | HTTP 200，恢复计数响应头为 `2`，响应 10240 B（SHA-256 `17eaddf7…`） | PASS |
| 5 | `batch_10` | POST 后 GET 轮询 | `/api/v1/batches` | 10 个虚构 TXT（10 文件批次） | `202 and 10 completed` | HTTP 202（响应 871 B）；终端摘要 `status=Completed`、`file_count=10`、`completed_count=10`、`failed_count=0`、`masked_entity_count=20` | PASS |
| 6 | `mixed_failure_detail` | POST 后 GET 轮询 | `/api/v1/batches` | 一正常 + 一损坏的混合批次（2 文件） | `CompletedWithErrors with safe file error` | HTTP 202（响应 214 B）；终端摘要 `batch_status=CompletedWithErrors`、`error_code=INPUT_CORRUPTED`，逐文件错误信息保持安全结构 | PASS |
| 7 | `missing_files` | POST | `/api/v1/batches` | 不带 `files` 字段 | `400 FILES_REQUIRED` | HTTP 400，`error_code=FILES_REQUIRED`（85 B） | PASS |
| 8 | `missing_rules` | POST | `/api/v1/batches` | 带 `files` 但不带 `rule_ids` | `400 INVALID_RULES` | HTTP 400，`error_code=INVALID_RULES`（76 B） | PASS |
| 9 | `invalid_rules` | POST | `/api/v1/batches` | `rule_ids` 含不受支持的规则 ID | `400 INVALID_RULES` | HTTP 400，`error_code=INVALID_RULES`（93 B） | PASS |
| 10 | `unexpected_field` | POST | `/api/v1/batches` | 合法字段之外附未知字段 | `400 UNEXPECTED_FIELD` | HTTP 400，`error_code=UNEXPECTED_FIELD`（90 B） | PASS |
| 11 | `unsupported_format` | POST | `/api/v1/batches` | 1 个扩展名/格式不受支持的文件 | `400 INPUT_FORMAT_UNSUPPORTED` | HTTP 400，`error_code=INPUT_FORMAT_UNSUPPORTED`（99 B） | PASS |
| 12 | `file_count_limit` | POST | `/api/v1/batches` | 101 个文件，超过默认文件数上限 | `413 INPUT_LIMIT_EXCEEDED` | HTTP 413，`error_code=INPUT_LIMIT_EXCEEDED`（96 B） | PASS |
| 13 | `missing_batch` | GET | `/api/v1/batches/{不存在的 batch_id}` | 随机 UUID 路径参数 | `404 NOT_FOUND` | HTTP 404，`error_code=NOT_FOUND`（87 B，SHA-256 `9bd50dd9…`） | PASS |
| 14 | `missing_artifact` | GET | `/api/v1/artifacts/{不存在的 artifact_id}` | 随机 UUID 路径参数 | `404 NOT_FOUND` | HTTP 404，`error_code=NOT_FOUND`（87 B，SHA-256 与第 13 项相同） | PASS |
| 15 | `restart_batch_persistence` | GET（SIGTERM 后重启） | `/api/v1/batches/{batch_id}` | 终态批次；对真实进程 SIGTERM，同数据目录重启后查询 | `200 Completed after restart` | HTTP 200，批次仍为 `Completed`（566 B） | PASS |
| 16 | `restart_download_persistence` | GET（重启后） | `/api/v1/artifacts/{artifact_id}` | 重启前已下载的产物 | `200 masked artifact after restart` | HTTP 200，响应 10233 B，SHA-256 与重启前 `artifact_download` 一致（`5b09fe0c…`） | PASS |
| 17 | `restart_restore_persistence` | POST（重启后） | `/api/v1/artifacts/{artifact_id}/restore` | 重启前已恢复的产物 | `200 restore after restart` | HTTP 200，响应 10240 B，SHA-256 与重启前 `artifact_restore` 一致（`17eaddf7…`） | PASS |

对应 [`TEST_PLAN.md`](./TEST_PLAN.md) P0-3 范围第 1-8 条；该计划第 9 条
（默认单文件/批次字节上限）按计划约定由当前 HEAD 的 Runtime 全量测试中的
限值注入用例验证，不属于本 17 项 API 断言范围。

## 5. 摘要字段交叉核对

以下一致性观察全部来自已提交摘要字段，不需要重建报文：

1. 第 16/17 项与第 3/4 项响应 SHA-256 逐字相同，支持"重启后产物与恢复结果
   未变化"的结论。
2. 第 13/14 项两条 404 响应字节数与 SHA-256 相同，与统一消毒错误结构一致。
3. 17 项中 13 项含 `safe_error_shape` 字段且全部为 `true`；其余 4 项为
   HTTP 200 的下载/恢复成功响应，不适用错误结构断言。
4. `error_code` 非空的 8 项（第 7-14 项）HTTP 状态与错误码全部与预期一致；
   HTTP 202 提交用例顶层 `error_code` 为 null，4 个 HTTP 200 下载/恢复成功用例未记录该字段。
5. 第 6 项 HTTP 状态仍为 202：混合批次按设计异步受理，部分失败体现在轮询
   终态 `CompletedWithErrors` 与逐文件安全错误信息中，而不是提交被拒绝。

## 6. 边界与外推限制

1. 本轮为单一 macOS 本机（loopback + 隔离测试实例）上的黑盒验收，**不代表
   Linux、Nginx、客户服务器或生产 SLA**。
2. 结果只覆盖 TXT 纯文本场景与上述接口；Office/PDF/OCR、高敏感信息密度、
   多实例、多租户等场景未覆盖，结论不得外推。
3. 第 4、17 项使用的 `POST /api/v1/artifacts/{artifact_id}/restore` 在
   [`API_REFERENCE`](../../enterprise/API_REFERENCE.md) 中被标注为"存在但
   不在首版承诺范围"，其通过仅证明该接口在本轮可用，不构成对该接口字段、
   稳定性或行为的任何承诺。
4. 浏览器/桌面 GUI/平台级测试：`NOT_RUN`。

## 7. 如何复核

- 对照本报告第 4 节断言表，逐项核对
  [`data/functional-results.json`](./data/functional-results.json) 的
  `case`/`expected`/`result` 与各断言字段是否一致。
- 本机并发性能结果与复算口径见
  [`PERFORMANCE_TEST_REPORT.md`](./PERFORMANCE_TEST_REPORT.md)。
