# Runtime API 功能与本机并发性能测试方案

> 工程内交付版，整理自内部验收任务
> `TASK-RUNTIME-API-FUNCTIONAL-PERFORMANCE-ACCEPTANCE-001` 已通过 Review 的测试方案。
> 正式结果见 [`FUNCTIONAL_TEST_REPORT.md`](./FUNCTIONAL_TEST_REPORT.md) 与
> [`PERFORMANCE_TEST_REPORT.md`](./PERFORMANCE_TEST_REPORT.md)。

## 目标

在产品基线 HEAD `622b8d73ccf8000f446ae4a440905f898a69ba07` 上，以真实独立
Runtime 进程验证 API 功能闭环，并形成当前 macOS 本机的容量观察。

**该结果不代表 Linux、Nginx、客户服务器或生产 SLA。**

## 环境与安全边界

- Runtime 仅监听 `127.0.0.1`（本轮使用随机选定端口的隔离测试实例，记录端口为
  18787；客户部署默认端口 8787 见
  [`../../enterprise/API_REFERENCE.md`](../../enterprise/API_REFERENCE.md)）。
- 每轮使用随机隔离的临时数据目录；结束后精确停止进程 PID、确认端口释放并删除
  临时目录。
- 仅生成虚构 UTF-8 TXT 输入；不读取用户文件，不配置 FileBay，不使用 Token、PIN、
  Keychain 或真实口令。
- 证据只保留状态、错误码、尺寸、计时、资源值和 SHA-256，不保存测试原文或恢复
  明文。

## P0-3 功能范围

1. health。
2. 单文件提交、轮询、下载、服务器侧恢复。
3. 10 文件批量提交与终态聚合。
4. 一成功一失败的混合批次及逐文件安全错误信息。
5. 无文件、缺少/非法规则、未知字段、不支持格式。
6. 101 文件超过默认文件数上限。
7. 不存在 batch/artifact。
8. 终态批次在真实进程 SIGTERM、同数据目录重启后仍可查询、下载、恢复。
9. 默认单文件/批次字节上限由当前 HEAD 的 Runtime 全量测试中的限值注入用例验证；
   不创建 500 MiB 单文件或 2 GiB 单批夹具。

## P0-4 性能范围

每组先预热 1 次（不计入样本），再正式测量 3 波。`C` 表示同时提交的批次数，
不代表 Runtime worker 数。

| 点位 | 单文件 | 每批文件 | C | 并发总输入 |
|---|---:|---:|---:|---:|
| M1 | 10 KiB | 1 | 1 | 10 KiB |
| M2 | 1 MiB | 1 | 1 | 1 MiB |
| M3 | 10 MiB | 1 | 1 | 10 MiB |
| M4 | 1 MiB | 10 | 1 | 10 MiB |
| M5 | 1 MiB | 50 | 1 | 50 MiB |
| M6 | 1 MiB | 10 | 2 | 20 MiB |
| M7 | 1 MiB | 10 | 4 | 40 MiB |
| M8 | 10 MiB | 10 | 2 | 200 MiB |
| M9 | 10 MiB | 50 | 1 | 500 MiB |

由此每组正式测量批次样本数为 `3 × C`（M6 = 6、M7 = 12、M8 = 6，其余为 3），
9 组合计 42 个批次样本。

## 指标与判定

- 记录 submit、从发起 POST 到观察到终态的 e2e、波次总耗时、文件/字节吞吐、
  提交/批次/文件成功率、P50/P95、Runtime CPU 均值/峰值和 RSS 峰值。
- 3 轮及并发批次全部 HTTP 202、全部文件 Completed、无 5xx/超时/崩溃且 RSS 未达到
  物理内存 50%，记为 `STABLE`。
- 任一点出现失败、资源门限或 300 秒单批超时，停止后续更高压力点并如实记录。
- 每组样本量有限，P95 接近最大观察值，仅用于本机比较，不作为生产统计承诺。

## 样本口径

- P50/P95 采用 nearest-rank 小样本口径：对某指标的 `n` 个批次样本升序排列后，
  P50 取第 `⌈0.5·n⌉` 小、P95 取第 `⌈0.95·n⌉` 小；`n` 为该组批次样本数（3/6/12）。
- 预热与正式测量的边界：预热批次不写入
  [`data/performance-raw.csv`](./data/performance-raw.csv)，该文件只含正式测量
  波次样本，`warmup` 列恒为 `False`，`repeat` 列标记正式波次序号（1-3）。
- 完整复算公式见
  [`PERFORMANCE_TEST_REPORT.md`](./PERFORMANCE_TEST_REPORT.md) 的
  "样本口径与复算"一节。

## 工具与可复现性

- 驱动器为内部一次性测试脚本，不随本仓库交付；本目录只交付其安全化输出数据
  （[`data/`](./data/)）。
- 内部第一轮运行（R1）因并发 e2e 计时少计等待区间整体作废，仅保留为测试驱动
  修正证据，不用于任何正式结论；本目录全部数据来自修正计时后的第二轮（R2）。

## 明确不在范围

- 浏览器/桌面 GUI/平台级性能测试：`NOT_RUN`。
- 高敏感信息密度、Office/PDF/OCR、磁盘压力持续运行、多个 Runtime 实例、多租户、
  长期压测：未覆盖，相关结论不得外推。
