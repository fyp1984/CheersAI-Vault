# RELEASE RECORD - CheersAI-SDLC 发布与运维记录
> 版本: v<X.Y.Z> | 负责人 (RO): <email> | 发布日期: <YYYY-MM-DD>
> BASE-TEST: TEST-<TICKET>.md@<sha>

## 一、发布内容
- Git Tag: `v<X.Y.Z>` (commit <sha>)
- 关联需求: `<TICKET1, TICKET2...>`
- 亮点:
- 破坏性变更 (Breaking Changes):

## 二、灰度发布轨迹（三阶段）
| 阶段 | 流量比例 | 开始时间 (UTC+8) | 观察时长 | 错误率 | 动作 | 执行人 |
|---|---|---|---|---|---|---|
| Canary 1%  | 1%   | | 60+ min | | keep/rollback | |
| Canary 10% | 10%  | | 60+ min | | keep/rollback | |
| Canary 50% | 50%  | | 60+ min | | keep/rollback | |
| Full 100%  | 100% | | —       | | released     | |

## 三、回滚演练 & 预案（预演环境 ≥ 1 次成功）
- 预演环境执行时间: ____
- 回滚耗时基线: ____ 秒
- 回滚步骤脚本:
```
```

## 四、监控 & 告警
### 4.1 RED/USE 仪表盘链接
- Rate/Error/Duration (RED):
- Utilization/Saturation/Error (USE):
### 4.2 告警 SLA 配置
| 告警级别 | 响应目标 | 本次是否已配置 |
|---|---|---|
| P0 | ≤15 分钟 | |
| P1 | ≤60 分钟 | |

## 五、SLA 达标报告（发布后 24h 统计）
| 指标 | 目标 | 实际 | 是否达标 |
|---|---|---|---|
| 可用性 SLA | ≥ 99.9% | | |
| P0 告警响应 | ≤ 15min | | |
| P0 故障恢复 | ≤ 60min | | |

## 六、发布后复盘 (Postmortem 触发条件：若有 P0/P1 故障)
- 事件时间线:
- 根因 (5 Whys):
- 改进项 (Action Items):

## 七、发布审计哈希
- 本记录 SHA256: `<由 audit-writer.py 写入>`
- 上一版本发布记录 SHA256: `<prev_hash>`
