#!/usr/bin/env python3
"""
collect-harness-coverage.py — HE-7 (Harness Coverage ≥ Code Coverage)
----------------------------------------------------------------------
扫描 audit 日志，统计 G1/G2/G3/G4/G5 5 门 Harness 在最近 N 天的：
  * trigger_count（触发频次）
  * first_real_defect_caught_count（首次捕获真缺陷 = exit_code != 0 且 ticket 之前从未在该 gate fail 过）
  * trigger_rate（触发率 = 触发过的 ticket / 全部 ticket ≥ 90%）

输出：sdlc/audit/<date>/harness-coverage.md  +  JSON
"""
from __future__ import annotations

import argparse
import json
import time
from collections import defaultdict
from pathlib import Path

SDLC_ROOT = Path(__file__).resolve().parent.parent
AUDIT = SDLC_ROOT / "audit"
GATES = ["G1-PRD", "G2-DESIGN", "G3-CODE", "G4-TEST", "G5-RELEASE"]


def iter_records(limit_days: int):
    days = sorted([p for p in AUDIT.iterdir() if p.is_dir() and p.name != ".gitkeep"], reverse=True)[:limit_days]
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
    p.add_argument("--out", default=None, help="输出 harness-coverage.md 路径")
    p.add_argument("--json-out", default=None)
    p.add_argument("--limit-days", type=int, default=14, help="默认统计最近 14 天 (=2 周 1 sprint)")
    p.add_argument("--min-trigger-rate", type=float, default=0.90, help="HE-7 触发率阈值 默认 90%")
    args = p.parse_args()

    per_gate_triggers: dict[str, set[str]] = defaultdict(set)      # gate -> set(tickets)
    per_gate_defects: dict[str, set[str]] = defaultdict(set)       # gate -> set(tickets that first-failed here)
    seen_fail: set[tuple[str, str]] = set()                        # (gate, ticket) 是否已 fail 过（用于 first_defect 计数）
    all_tickets: set[str] = set()

    for r in iter_records(args.limit_days):
        g = r.get("gate", "")
        tid = r.get("ticket", "")
        if not g or not tid or g not in GATES:
            continue
        all_tickets.add(tid)
        per_gate_triggers[g].add(tid)
        if r.get("exit_code", 0) != 0 and (g, tid) not in seen_fail:
            per_gate_defects[g].add(tid)
            seen_fail.add((g, tid))

    total_tickets = max(1, len(all_tickets))
    rows = []
    for g in GATES:
        triggered_tickets = per_gate_triggers.get(g, set())
        defects = per_gate_defects.get(g, set())
        trigger_rate = len(triggered_tickets) / total_tickets
        rows.append({
            "gate": g,
            "triggered_ticket_count": len(triggered_tickets),
            "first_real_defect_caught": len(defects),
            "trigger_rate": round(trigger_rate, 4),
            "trigger_rate_pass": trigger_rate >= args.min_trigger_rate,
        })

    overall_pass = all(r["trigger_rate_pass"] for r in rows)
    lines = []
    lines.append("# Harness Coverage 报告（HE-7：Harness 覆盖率 ≥ 代码覆盖率）")
    lines.append("")
    lines.append(f"- 统计窗口：最近 {args.limit_days} 天")
    lines.append(f"- 全部门 {GATES}")
    lines.append(f"- 涉及 Ticket 总数: {len(all_tickets)}")
    lines.append(f"- HE-7 阈值：单门 Harness 触发率 ≥ {int(args.min_trigger_rate*100)}%（{int(args.min_trigger_rate*100)}% 的 Ticket 都在该门被校验过）")
    lines.append("")
    lines.append("| Gate | 触发过的 Tickets | 首次捕获真缺陷数 | 触发率 | 达标？ |")
    lines.append("|---|---|---|---|---|")
    for r in rows:
        marker = "✅" if r["trigger_rate_pass"] else "❌"
        lines.append(f"| {r['gate']} | {r['triggered_ticket_count']} | {r['first_real_defect_caught']} | {r['trigger_rate']*100:.1f}% | {marker} |")
    lines.append("")
    lines.append(f"## 总体结论")
    lines.append(f"- **{'✅ PASS' if overall_pass else '❌ FAIL'}** — Harness Coverage {'达到' if overall_pass else '未达到'} HE-7 阈值 ≥ {int(args.min_trigger_rate*100)}%。")
    if not overall_pass:
        lines.append(f"- 🚨 整改建议：下个 Sprint 增加 {[r['gate'] for r in rows if not r['trigger_rate_pass']]} 门的校验样本（补 Ticket），或 Sprint 复盘会说明无需覆盖的充分理由并经 TD+RO 双签豁免。")
    lines.append("")
    lines.append("（对应 HE-7：不仅代码覆盖率要看，每一门 Harness 自身也要有足够触发频次 & 捕获真缺陷的能力，杜绝『工装摆设』。）")
    text = "\n".join(lines) + "\n"

    import datetime
    today_dir = AUDIT / datetime.date.today().strftime("%Y%m%d")
    today_dir.mkdir(parents=True, exist_ok=True)
    md_path = Path(args.out) if args.out else (today_dir / "harness-coverage.md")
    json_path = Path(args.json_out) if args.json_out else (today_dir / "harness-coverage.json")
    md_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.parent.mkdir(parents=True, exist_ok=True)
    md_path.write_text(text, encoding="utf-8")
    payload = {
        "generated_at": int(time.time()),
        "limit_days": args.limit_days,
        "total_tickets": len(all_tickets),
        "min_trigger_rate": args.min_trigger_rate,
        "per_gate": rows,
        "overall_pass": overall_pass,
    }
    json_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"[collect-harness-coverage] ok -> MD={md_path}  JSON={json_path}")
    print(f"[collect-harness-coverage] overall_pass={overall_pass}")
    return 0 if overall_pass else 1


if __name__ == "__main__":
    raise SystemExit(main())
