#!/usr/bin/env python3
"""
gate-test.py — G4 门禁：解析 test-report JSON，验证核心场景通过率/缺陷修复率/安全 0 高危
用法:
  ./bin/gate-test.py --test-report artifacts/test-report.json --ticket IMP-001
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

SDLC_ROOT = Path(__file__).resolve().parent.parent


def audit(ticket, report, exit_code, evidence, actor):
    try:
        return subprocess.call(
            [
                sys.executable, str(SDLC_ROOT / "bin" / "audit-writer.py"),
                "--ticket", ticket, "--gate", "G4-TEST",
                "--actor_email", actor, "--role", "QA",
                "--artifact", str(report.resolve()),
                "--exit_code", str(exit_code),
                "--evidence", json.dumps(evidence, ensure_ascii=False),
            ], stdout=sys.stdout, stderr=sys.stderr,
        )
    except FileNotFoundError:
        return 0


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--test-report", required=True)
    p.add_argument("--ticket", required=True)
    p.add_argument("--actor", default="sdlc-qa@cheersai.ai")
    args = p.parse_args()

    report = Path(args.test_report).resolve()
    if not report.is_file():
        # 允许演示模式：没写 report 也能生成最小占位
        demo = input(f"[G4-TEST] 未找到 {report}，是否生成演示最小占位 test-report.json 继续? (y/N) ")
        if demo.strip().lower() != "y":
            return 1
        report.parent.mkdir(parents=True, exist_ok=True)
        demo_obj = {
            "functional": {"p0_total": 4, "p0_pass": 4, "p1_total": 3, "p1_pass": 3, "p2_total": 2, "p2_pass": 2},
            "defects":    {"p0_total": 0, "p0_fixed": 0, "p1_total": 0, "p1_fixed": 0, "p2_leftover": 0},
            "security":   {"sast_critical": 0, "sca_critical": 0, "dast_high": 0},
            "performance": {"tp99_regression_ratio": 1.02},
        }
        report.write_text(json.dumps(demo_obj, ensure_ascii=False, indent=2), encoding="utf-8")

    data = json.loads(report.read_text(encoding="utf-8"))
    f = data.get("functional", {})
    d = data.get("defects", {})
    s = data.get("security", {})
    perf = data.get("performance", {})

    p0_pass_rate = (f["p0_pass"] / f["p0_total"]) if f.get("p0_total") else 1.0
    p1_pass_rate = (f["p1_pass"] / f["p1_total"]) if f.get("p1_total") else 1.0
    p0_fix_rate  = (d["p0_fixed"] / d["p0_total"]) if d.get("p0_total") else 1.0
    p1_fix_rate  = (d["p1_fixed"] / d["p1_total"]) if d.get("p1_total") else 1.0

    checks = [
        ("P0 用例通过率 100%", p0_pass_rate >= 1.0),
        ("P1 用例通过率 100%", p1_pass_rate >= 1.0),
        ("P0 缺陷修复率 100%", p0_fix_rate  >= 1.0),
        ("P1 缺陷修复率 100%", p1_fix_rate  >= 1.0),
        ("P2 遗留 ≤ 3",        d.get("p2_leftover", 0) <= 3),
        ("SAST Critical = 0",  s.get("sast_critical", 1) == 0),
        ("SCA  Critical = 0",  s.get("sca_critical",  1) == 0),
        ("DAST High = 0",      s.get("dast_high",      1) == 0),
        ("TP99 回归 ≤ 10%",    perf.get("tp99_regression_ratio", 99) <= 1.10),
    ]

    all_ok = True
    for label, ok in checks:
        marker = "✅" if ok else "❌"
        if not ok: all_ok = False
        print(f"[G4-TEST] {marker} {label}")

    evidence = {
        "p0_pass_rate": round(p0_pass_rate, 4),
        "p1_pass_rate": round(p1_pass_rate, 4),
        "p0_fix_rate": round(p0_fix_rate, 4),
        "p1_fix_rate": round(p1_fix_rate, 4),
        "p2_leftover": d.get("p2_leftover", 0),
        "sast_critical": s.get("sast_critical", None),
        "sca_critical":  s.get("sca_critical", None),
        "dast_high":     s.get("dast_high", None),
        "tp99_regression_ratio": perf.get("tp99_regression_ratio", None),
    }
    overall = 0 if all_ok else 6
    print(f"[G4-TEST] -> {'PASS' if overall==0 else 'FAIL'}")
    audit(args.ticket, report, overall, evidence, args.actor)
    return overall


if __name__ == "__main__":
    raise SystemExit(main())
