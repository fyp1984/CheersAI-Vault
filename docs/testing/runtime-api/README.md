# Runtime API 测试资料（功能验收与本机性能基线）

本目录是 Runtime API 客户测试能力的工程内测试资料：测试方案、17 项功能断言结果、
9 组本机并发性能矩阵，以及安全化后的机器可读数据文件。

资料整理自 2026-09-04 的一轮独立本机验收（内部验收任务
`TASK-RUNTIME-API-FUNCTIONAL-PERFORMANCE-ACCEPTANCE-001` 的第二轮 R2 数据），
只做证据的工程化整理，不修改任何 Runtime/API 产品逻辑。

## 结论速览

- 功能：17 项黑盒功能断言全部 `PASS`（17 PASS / 0 FAIL / 0 BLOCKED / 0 NOT_RUN）。
- 性能：9 组压力点位全部 `STABLE`（macOS 本机、低命中密度 TXT 场景）。
- 边界：以上是单一 macOS 本机（loopback + debug Runtime）观察值，
  **不代表 Linux、Nginx、客户服务器或生产 SLA**。

## 文档导航

- 测试方案（范围、环境与安全边界、判定标准）：[`TEST_PLAN.md`](./TEST_PLAN.md)
- 功能测试报告（17 项断言摘要）：[`FUNCTIONAL_TEST_REPORT.md`](./FUNCTIONAL_TEST_REPORT.md)
- 性能测试报告（9 组矩阵与复算口径）：[`PERFORMANCE_TEST_REPORT.md`](./PERFORMANCE_TEST_REPORT.md)
- 客户测试 API 参考：[`../../enterprise/API_REFERENCE.md`](../../enterprise/API_REFERENCE.md)
- 机器可读数据：[`data/`](./data/)

## 证据边界（必读）

1. **功能数据是断言摘要，不是完整 HTTP 报文存档。** 原始请求/响应正文从未保留，
   存档字段仅包括 HTTP 状态、错误码、安全错误结构标志、响应尺寸、响应 SHA-256
   与断言结论，无法据此重建原始报文（见
   [`FUNCTIONAL_TEST_REPORT.md`](./FUNCTIONAL_TEST_REPORT.md)）。
2. **性能样本只含正式测量波次。** 每组先预热 1 次、再正式测量 3 波；预热不计入
   样本，[`data/performance-raw.csv`](./data/performance-raw.csv) 恰好是 42 个
   正式测量批次样本，`warmup` 列恒为 `False`。
3. **不提交不可复算资源字段。** 原始资源采样文件没有 warmup/repeat/wave 标签，
   无法区分预热与正式波次，因此资源明细不随工程提交；CPU/RSS 汇总也无法从
   已提交数据独立复算，已从交付版汇总 JSON 和报告中移除具体数值（详见
   [`PERFORMANCE_TEST_REPORT.md`](./PERFORMANCE_TEST_REPORT.md)）。
4. **安全化。** 本目录不含凭据、真实敏感数据、恢复明文或本机绝对路径；测试输入
   全部为虚构 UTF-8 TXT，随机批次标识已替换为确定性样本标识。
5. **覆盖范围。** 功能与性能结果只覆盖 TXT 纯文本场景与
   [`API_REFERENCE`](../../enterprise/API_REFERENCE.md) 承诺的四项 API（另含一项
   被标注为"存在但不在首版承诺范围"的服务器映射恢复接口），不代表
   Office/PDF/OCR 等格式能力。

## 数据文件

| 文件 | 内容 | 口径 |
|---|---|---|
| [`data/functional-results.json`](./data/functional-results.json) | 17 项功能断言摘要 | 与内部 R2 记录逐字一致 |
| [`data/performance-raw.csv`](./data/performance-raw.csv) | 42 个正式测量批次样本 | 随机 `batch_id` 已替换为确定性 `sample_id`（规则见性能报告） |
| [`data/performance-summary.json`](./data/performance-summary.json) | 9 组性能汇总 | 只保留可由提交 CSV 复算的字段；CPU/RSS 字段未提交 |
| [`data/machine-info.json`](./data/machine-info.json) | 测试机器与候选 Runtime 身份 | 与内部 R2 记录逐字一致 |

## 如何复核

- 功能：对照 [`FUNCTIONAL_TEST_REPORT.md`](./FUNCTIONAL_TEST_REPORT.md) 的断言表，
  逐项核对 [`data/functional-results.json`](./data/functional-results.json) 的
  结论与字段。
- 性能：按 [`PERFORMANCE_TEST_REPORT.md`](./PERFORMANCE_TEST_REPORT.md)
  "样本口径与复算"一节的公式，从
  [`data/performance-raw.csv`](./data/performance-raw.csv) 一次性只读复算
  submit/e2e 的 P50/P95、成功率与文件/字节吞吐，并与
  [`data/performance-summary.json`](./data/performance-summary.json) 比对；
  CPU/RSS 字段按上述第 3 条边界不予提交。
