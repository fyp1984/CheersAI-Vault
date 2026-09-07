# Runtime API 本机并发性能测试报告（9 组矩阵）

> 工程内交付版，整理自内部验收任务
> `TASK-RUNTIME-API-FUNCTIONAL-PERFORMANCE-ACCEPTANCE-001` 第二轮（R2）的
> 本机并发性能数据。测试方案（点位设计、判定标准、样本口径）见
> [`TEST_PLAN.md`](./TEST_PLAN.md)；机器可读数据见
> [`data/performance-raw.csv`](./data/performance-raw.csv)（42 个正式测量
> 批次样本）与 [`data/performance-summary.json`](./data/performance-summary.json)。

## 1. 结论速览

- 9 组压力点位（M1-M9）全部 `STABLE`：9/9。
- 42/42 个正式测量批次全部 HTTP 202、终态 `Completed`、`failed_files=0`；
  复算文件成功率 579/579 = 100%。
- 无 5xx、无单批 300 秒超时、无进程崩溃。
- e2e（发起 POST 到观察终态）最大观察值 7921.853 ms（M9），最小 128.586 ms
  （M1）。
- 边界：全部数据来自单一 macOS 本机（loopback + 隔离测试实例），**不代表
  Linux、Nginx、客户服务器或生产 SLA**（详见第 8 节）。

## 2. 测试机器与被测对象

| 项目 | 值 |
|---|---|
| 系统 | Darwin 25.6.0（macOS），arm64 |
| 机型 / 芯片 | Mac14,2 / Apple M2，8 逻辑核 |
| 物理内存 | 17,179,869,184 B（16 GiB） |
| 磁盘 | 总量 494,384,795,648 B（约 460.4 GiB），测试开始时可用 172,954,263,552 B（约 161.0 GiB） |
| 产品基线 HEAD | `622b8d73ccf8000f446ae4a440905f898a69ba07` |
| Runtime 二进制 | 91,597,200 B，SHA-256 `4598762941a761f54216dd477f72ce30eec24a3ebd4f54eeb5847a4b3f220909` |
| 测试驱动器 Python | 3.13.3（一次性内部脚本，不随仓库交付） |
| 实例形态 | Runtime 仅监听 `127.0.0.1`，随机选定端口的隔离实例（记录端口 18787）；每轮随机临时数据目录，结束后停止 PID 并清理 |

以上机器元数据见 [`data/machine-info.json`](./data/machine-info.json)。

## 3. 执行口径：预热边界与 42 个批次样本

1. **波次边界**：每组点位先**预热 1 次**，再**正式测量 3 波**（`repeat` 列
   1/2/3）。预热批次不计入任何样本，也不写入
   [`data/performance-raw.csv`](./data/performance-raw.csv)。
2. **样本数**：每组正式样本数 = 3 波 × C（同时提交的批次数，不代表 Runtime
   worker 数）。合计 3+3+3+3+3+6+12+6+3 = **42 个批次样本**，与 CSV 行数
   一致；CSV `warmup` 列恒为 `False`。
3. **样本标识**：原始随机批次标识不落盘，`sample_id` 为确定性标识
   `<点位>-R<波次>-S<波内批次序号>`（如 `M7-R2-S03`）。
4. **计时口径**：`submit_s` 为 POST 响应返回耗时；`e2e_s` 为从发起 POST 到
   观察到该批次终态的耗时；`wave_s` 为波次总耗时（发起该波全部批次到全部
   批次到达终态的墙钟时间）。`C > 1` 时同一波次内各并发批次的 `wave_s`
   相同（共享同一墙钟区间）。
5. **停止规则**：任一点出现失败、资源门限或 300 秒单批超时即停止后续更高
   压力点并如实记录；本轮 9 组全部执行完毕，停止规则未触发。

## 4. 样本口径与复算

### 4.1 参与复算的字段

[`data/performance-raw.csv`](./data/performance-raw.csv) 全部 14 个数据字段
（`sample_id`、`completed_files`、`e2e_s`、`error_code`、`failed_files`、
`file_count`、`http_status`、`input_bytes`、`point`、`repeat`、`submit_s`、
`terminal_status`、`warmup`、`wave_s`）均可参与复算。

### 4.2 nearest-rank 百分位

P50/P95 采用 nearest-rank 小样本口径：对某指标的 `n` 个批次样本升序排列后，
P50 取第 `⌈0.5·n⌉` 小、P95 取第 `⌈0.95·n⌉` 小；`n` 为该组批次样本数
（3/6/12）。`n=3` 时 P50 为第 2 小、P95 即最大观察值，因此 **P95 接近最大
观察值，仅用于本机横向比较，不作为生产统计承诺**。

### 4.3 复算公式

对每个点位（组内批次行数为 `n = 3×C`）：

- `submit_p50_ms / submit_p95_ms`：对 `submit_s × 1000` 按 4.2 取分位。
- `e2e_p50_ms / e2e_p95_ms`：对 `e2e_s × 1000` 按 4.2 取分位。
- `file_success_rate`：`Σcompleted_files / Σfile_count`。
- 波次墙钟合计：`W = Σ_wave(wave_s)`——**按波次去重求和**（同一
  `(point, repeat)` 波次内 C 个并发批次共享同一 `wave_s`，只计一次）。
  `C = 1` 的组与按行求和等价；`C > 1` 的组若按行求和会把墙钟重复计入 C 倍，
  得到错误吞吐。
- `completed_files_per_s`：`Σcompleted_files / W`。
- `completed_mib_per_s`：`Σinput_bytes / 1048576 / W`。

复算示例（M7，C=4，n=12）：`W = 0.9884598 + 0.9672624 + 0.9810002 =
2.9367224 s`；`Σcompleted_files = 120`；`120 / 2.9367224 = 40.8619
files/s`，与 [`data/performance-summary.json`](./data/performance-summary.json)
一致。

### 4.4 复算核对结果

整理时按上述公式对 9 组逐组一次性只读复算，`samples_n`、四项 P50/P95、
`file_success_rate`、`completed_files_per_s`、`completed_mib_per_s` 与
[`data/performance-summary.json`](./data/performance-summary.json) 全部一致。

### 4.5 CPU/RSS：不提交不可复算字段（必读）

- 原始资源采样明细文件（无标签 `resource-samples.csv`）缺少
  warmup/repeat/wave 标签，无法区分预热与正式波次，因此**未随工程提交**；
  本目录与 [`data/`](./data/) 下不存在该文件。
- 原验收运行产生过 CPU/RSS 汇总观察值，但这些字段无法从已提交数据独立复算，
  因而本交付版也从 [`data/performance-summary.json`](./data/performance-summary.json)
  删除 `cpu_mean_percent`、`cpu_peak_percent`、`rss_peak_mib`，不发布具体资源数值，
  不用不可复算字段证明资源门限或瓶颈。

## 5. 9 组结果矩阵

矩阵点位（单文件大小 × 每批文件数 × C）与正式测量结果：

| 点位 | 单文件 | 每批文件 | C | 并发总输入 | 样本 n | submit P50/P95 (ms) | e2e P50/P95 (ms) | 文件吞吐 (files/s) | 字节吞吐 (MiB/s) | 结果 |
|---|---:|---:|---:|---:|---:|---|---|---:|---:|---|
| M1 | 10 KiB | 1 | 1 | 10 KiB | 3 | 23.339 / 23.418 | 130.588 / 134.147 | 7.485 | 0.073 | STABLE |
| M2 | 1 MiB | 1 | 1 | 1 MiB | 3 | 25.174 / 26.637 | 133.848 / 137.176 | 7.361 | 7.361 | STABLE |
| M3 | 10 MiB | 1 | 1 | 10 MiB | 3 | 60.599 / 61.115 | 170.078 / 170.566 | 5.869 | 58.686 | STABLE |
| M4 | 1 MiB | 10 | 1 | 10 MiB | 3 | 81.526 / 82.459 | 297.442 / 298.420 | 33.526 | 33.526 | STABLE |
| M5 | 1 MiB | 50 | 1 | 50 MiB | 3 | 346.522 / 403.299 | 1274.617 / 1488.387 | 38.788 | 38.788 | STABLE |
| M6 | 1 MiB | 10 | 2 | 20 MiB | 6 | 91.102 / 98.378 | 414.030 / 553.770 | 43.291 | 43.291 | STABLE |
| M7 | 1 MiB | 10 | 4 | 40 MiB | 12 | 100.055 / 334.622 | 670.771 / 984.377 | 40.862 | 40.862 | STABLE |
| M8 | 10 MiB | 10 | 2 | 200 MiB | 6 | 581.458 / 612.247 | 1862.287 / 2724.109 | 7.692 | 76.921 | STABLE |
| M9 | 10 MiB | 50 | 1 | 500 MiB | 3 | 2677.886 / 2687.404 | 7902.866 / 7921.853 | 6.334 | 63.339 | STABLE |

## 6. 稳定边界

源测试驱动的 `STABLE` 判定包含：3 轮及并发批次全部 HTTP 202、全部文件
`Completed`、无 5xx/超时/崩溃，且 RSS 未达到物理内存 50%。本交付数据可以
独立复算请求、文件、延迟和吞吐部分；由于第 4.5 节所述标签缺口，不能独立复算
RSS 门限，矩阵中的 `STABLE` 应理解为源驱动记录结论，而非仅凭提交数据重新证明。

本轮 9 组的实测边界：

1. 42/42 批次 `http_status=202`、`terminal_status=Completed`、
   `failed_files=0`、`error_code` 全为空；文件成功率复算 579/579 = 100%。
2. 最大 e2e 7921.853 ms（M9），距 300 秒单批超时门限有约 38 倍余量。
3. 无失败、无进程重启或崩溃记录；资源门限只能保留为源驱动结论，不能由本交付
   数据独立复算。

## 7. 瓶颈观察

以下为对本机数据的观察，全部可由
[`data/performance-raw.csv`](./data/performance-raw.csv) 与
[`data/performance-summary.json`](./data/performance-summary.json) 支撑，
**不得外推到其他硬件或生产环境**：

1. **小文件受固定开销主导**：单文件从 10 KiB（M1）增大到 10 MiB（M3），
   e2e 仅从 130.588 ms 增至 170.078 ms，字节吞吐却从 0.073 升至
   58.686 MiB/s——小文件的耗时几乎全部是每请求固定开销。
2. **批内文件数摊薄固定开销**：同为 1 MiB 文件，每批 1 个（M2，e2e
   133.848 ms、7.361 files/s）到每批 10 个（M4，e2e 297.442 ms、
   33.526 files/s）再到每批 50 个（M5，38.788 files/s），文件吞吐显著上升；
   代价是提交耗时增长（M5 submit P50 346.522 ms）。
3. **并发收益在 C=2 后耗尽，处理路径出现饱和迹象**：C 从 1（M4，33.526 MiB/s）
   提到 2（M6，43.291 MiB/s）仍有收益；继续提到 4（M7，40.862 MiB/s）不再
   提升，e2e P95 从 553.770 ms 拉宽到 984.377 ms。提交数据能证明吞吐收益已经
   见顶和等待时间增加，但不能在缺少可复算资源样本时进一步断言 CPU 或内存瓶颈。
4. **大文件高并发吞吐**：M8（10 MiB × 10 文件 × C=2）字节吞吐全矩阵最高
   （76.921 MiB/s）。
5. **大批次延迟**：M9（500 MiB / 50 文件单批）submit P50 2677.886 ms、
   e2e P50 7902.866 ms，是本轮延迟最高的点位。

## 8. macOS loopback debug 限制与外推红线

1. **单一测试机**：全部数据来自一台 Apple M2 / 8 逻辑核 / 16 GiB 内存的
   macOS 本机；单机观察值不具备跨环境代表性。
2. **loopback 网络**：Runtime 仅监听 `127.0.0.1`，请求不经过 Nginx、TLS、
   局域网 RTT 与白名单过滤；生产链路（客户内网系统 → Nginx → Runtime）的
   开销与失败模式完全不在本数据内。
3. **隔离 debug 实例**：被测对象是随机端口、随机临时数据目录的本机隔离
   实例，不是 Linux 生产部署形态；本轮**不代表 Linux、Nginx、客户服务器或
   生产 SLA**，不构成任何容量承诺。
4. **小样本**：每组样本量 3/6/12，P95 即接近最大观察值（见第 4.2 节），
   数值波动敏感，仅用于本机横向比较。
5. **未覆盖场景**：高敏感信息密度、Office/PDF/OCR、磁盘压力持续运行、
   多个 Runtime 实例、多租户、长期压测均未覆盖；浏览器/桌面 GUI/平台级
   性能测试 `NOT_RUN`。
6. **资源采样边界**：原始资源采样明细没有波次标签，CPU/RSS 明细和汇总数值
   均未提交（见第 4.5 节）。

## 9. 数据文件与复核方法

| 文件 | 内容 | 口径 |
|---|---|---|
| [`data/performance-raw.csv`](./data/performance-raw.csv) | 42 个正式测量批次样本 | 仅正式波次；`warmup` 恒为 `False`；随机 `batch_id` 已替换为确定性 `sample_id` |
| [`data/performance-summary.json`](./data/performance-summary.json) | 9 组汇总 | 仅保留第 4.3 节可复算字段；未提交 CPU/RSS 字段 |
| [`data/machine-info.json`](./data/machine-info.json) | 测试机器与候选 Runtime 身份 | 机器元数据 |

复核方法（一次性只读，无需运行 Runtime）：用任意 CSV 读取器载入
[`data/performance-raw.csv`](./data/performance-raw.csv)，按第 4.3 节公式
逐组计算并与 [`data/performance-summary.json`](./data/performance-summary.json)
比对；注意并发组的 `wave_s` 必须按波次去重求和。功能侧 17 项断言见
[`FUNCTIONAL_TEST_REPORT.md`](./FUNCTIONAL_TEST_REPORT.md)。
