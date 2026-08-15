#!/usr/bin/env python3
"""
retrospective-feedback.py — Harness Engineering HE-6 (反馈驱动 Harness 进化)
每次 Release 后分析所有 G1-G5 的审计条目，识别：
  * 连续 3 次 100% 通过且返工 <1 的非核心检查项 → 候选降级
  * 连续 2 次失败的检查项 → 候选加强
输出 process-redundancy-report.md
"""
from __future__ import annotations

import argparse
import json
from collections import defaultdict
from pathlib import Path

SDLC_ROOT = Path(__file__).resolve().parent.parent
AUDIT = SDLC_ROOT / "audit"


def iter_records(limit_days: int = 10):
    days = sorted([p for p in AUDIT.iterdir() if p.is_dir()], reverse=True)[:limit_days]
    for d in days:
        for f in sorted(d.glob("_*.jsonl")):
            for line in f.open("r", encoding="utf-8"):
                line = line.strip()
                if not line:
                    continue
                try:
                    yield json.loads(line)
                except json.JSONDecodeError:
                    continue


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--out", default=None)
    p.add_argument("--limit-days", type=int, default=10)
    args = p.parse_args()

    per_gate_ticket: dict[str, dict[str, list[dict]]] = defaultdict(lambda: defaultdict(list))
    for r in iter_records(args.limit_days):
        per_gate_ticket[r["gate"]][r["ticket"]].append(r)

    # 统计每个 gate 在 ticket 维度的 pass/fail 序列
    per_gate_seq: dict[str, list[bool]] = defaultdict(list)
    for gate, tickets in per_gate_ticket.items():
        for tid, recs in tickets.items():
            # 同一 ticket 同一 gate，只要最后一条 exit_code=0 就视为最终 pass
            final = sorted(recs, key=lambda x: x["ts_unix"])[-1]
            per_gate_seq[gate].append(final["exit_code"] == 0)

    report_lines = []
    report_lines.append("# 流程冗余度与改进建议报告 (Process Redundancy Report)")
    report_lines.append("")
    report_lines.append("本报告由 Harness Engineering HE-6 (反馈驱动 Harness 进化) 自动生成。")
    report_lines.append("")
    report_lines.append("## 每 G 步通过率（近 %d 天）" % args.limit_days)
    for g, seq in sorted(per_gate_seq.items()):
        pass_rate = sum(1 for x in seq if x) / len(seq) if seq else float("nan")
        report_lines.append(f"- {g}: {pass_rate*100:.1f}% ({sum(1 for x in seq if x)}/{len(seq)})")

    report_lines.append("")
    report_lines.append("## 候选建议")
    action_taken = False
    for g, seq in per_gate_seq.items():
        # 连续 3 次全通过且样本≥3 → 候选降级（非核心检查项）
        if len(seq) >= 3 and all(seq[-3:]):
            report_lines.append(f"- 💡 **{g}**：连续 3 次最终 PASS，可评估是否将部分检查项移入夜间巡检，缩短研发前置时间。")
            action_taken = True
        # 最近 2 次 fail
        if len(seq) >= 2 and (not seq[-1]) and (not seq[-2]):
            report_lines.append(f"- 🚨 **{g}**：最近 2 次最终 FAIL，建议补充该 G 步检查项的自动化校验或提前介入评审。")
            action_taken = True
    if not action_taken:
        report_lines.append("- ✅ 近期各环节稳定性良好，暂无需优化建议。")

    text = "\n".join(report_lines) + "\n"
    out_path: Path
    if args.out:
        out_path = Path(args.out)
    else:
        import time
        d = AUDIT / time.strftime("%Y%m%d", time.localtime())
        d.mkdir(parents=True, exist_ok=True)
        out_path = d / "process-redundancy-report.md"
    out_path.write_text(text, encoding="utf-8")
    print(f"[retrospective-feedback] ok -> {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
