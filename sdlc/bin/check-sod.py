#!/usr/bin/env python3
"""
check-sod.py — Harness Engineering HE-4 (Zero-Trusted Pass-Thru + SoD)
------------------------------------------------------------
验证同一 TICKET 下没有一人兼任 RA+TD / TD+CD / CD+QA / QA+RO。

读取今天的 session audit 日志，按 actor_email 聚合所有出现过的 role，
若命中禁止组合 → exit 非 0。
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from collections import defaultdict
from pathlib import Path

SDLC_ROOT = Path(__file__).resolve().parent.parent
AUDIT_ROOT = SDLC_ROOT / "audit"

FORBIDDEN_PAIRS = [
    {"RA", "TD"},
    {"TD", "CD"},
    {"CD", "QA"},
    {"QA", "RO"},
]


def today_sessions(ticket: str):
    d = AUDIT_ROOT / time.strftime("%Y%m%d", time.localtime())
    if not d.exists():
        return
    for f in sorted(d.glob("_*.jsonl")):
        with f.open("r", encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    r = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if r.get("ticket") == ticket:
                    yield r


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--ticket", required=True)
    args = p.parse_args()

    per_actor: dict[str, set[str]] = defaultdict(set)
    for r in today_sessions(args.ticket):
        per_actor.setdefault(r["actor_email"], set()).add(r["role"])

    violations = []
    for email, roles in per_actor.items():
        for forbidden in FORBIDDEN_PAIRS:
            if forbidden.issubset(roles):
                violations.append((email, sorted(forbidden), sorted(roles)))

    if violations:
        print(f"[check-sod] ❌ SoD 违规 ({len(violations)} 条):")
        for email, forbid, all_roles in violations:
            print(f"   - {email}: 禁止组合 {forbid}，实际担任 {all_roles}")
        return 2
    print(f"[check-sod] ✅ Ticket={args.ticket} SoD 通过，参与角色: {dict((e,sorted(r)) for e,r in per_actor.items())}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
