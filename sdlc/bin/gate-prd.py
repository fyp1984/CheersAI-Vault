#!/usr/bin/env python3
"""
gate-prd.py — G1 门禁：PRD 完整性 + 合规性双维度扫描
- completeness: G1-PRD.spec.yaml 要求的 6 大章节必须齐全
- compliance: 若有 PII 字段，必须标注 C3/C4 + 脱敏策略
"""
from __future__ import annotations

import argparse
import hashlib
import re
import subprocess
import sys
from pathlib import Path

SDLC_ROOT = Path(__file__).resolve().parent.parent
REQUIRED_SECTIONS = [
    "一、背景与目标",
    "二、用户故事与核心场景",
    "三、验收标准（AC，可量化）",
    "四、优先级（MoSCoW",
    "五、依赖关系 & 上下游影响",
    "六、合规与数据分级",
]

PII_HINT_RE = re.compile(r"手机号|身份证号|邮箱|银行卡|护照|住址|地址|真实姓名|PII|敏感")
MASK_HINT_RE = re.compile(r"脱敏|masking|加密|encryption|at-?rest|in-?transit")


def sha256_of(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def run_audit(ticket: str, prd_path: Path, exit_code: int, evidence: dict, actor: str) -> int:
    try:
        return subprocess.call(
            [
                sys.executable, str(SDLC_ROOT / "bin" / "audit-writer.py"),
                "--ticket", ticket,
                "--gate", "G1-PRD",
                "--actor_email", actor,
                "--role", "RA",
                "--artifact", str(prd_path.resolve()),
                "--exit_code", str(exit_code),
                "--evidence", __import__("json").dumps(evidence, ensure_ascii=False),
            ],
            stdout=sys.stdout, stderr=sys.stderr,
        )
    except FileNotFoundError:
        return 0


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--prd", required=True, help="PRD md 路径")
    p.add_argument("--ticket", default="AUTO", help="若为 AUTO 则从文件名 PRD-XXX.md 提取")
    p.add_argument("--actor", default="sdlc-ra@cheersai.ai")
    args = p.parse_args()

    prd = Path(args.prd).resolve()
    if not prd.is_file():
        print(f"[G1-PRD] ❌ 找不到 PRD 文件: {prd}")
        return 1
    text = prd.read_text(encoding="utf-8")

    ticket = args.ticket
    if ticket == "AUTO":
        m = re.match(r"PRD-([A-Za-z0-9_\-]+)\.md$", prd.name)
        if m:
            ticket = m.group(1)

    # 1) Completeness
    missing = [s for s in REQUIRED_SECTIONS if s not in text]
    completeness = 1.0 if not missing else (len(REQUIRED_SECTIONS) - len(missing)) / len(REQUIRED_SECTIONS)
    if missing:
        print(f"[G1-PRD] ❌ 缺失章节: {missing}")

    # 2) Compliance (C3/PII)
    has_pii_hint = bool(PII_HINT_RE.search(text))
    has_masking = bool(MASK_HINT_RE.search(text))
    compliance = 1.0
    if has_pii_hint and not has_masking:
        print("[G1-PRD] ❌ 出现 PII 字段(手机号/身份证号/邮箱/住址等) 但未定义脱敏/加密策略")
        compliance = 0.0

    # 3) BASE 字段（PRD 本身要能导出 sha，供 DESIGN 引用）
    sha = sha256_of(prd)
    print(f"[G1-PRD] PRD-SHA256={sha}")
    evidence = {
        "prd_completeness_pct": round(completeness * 100, 1),
        "prd_compliance_pct": round(compliance * 100, 1),
        "prd_has_pii_hint": has_pii_hint,
        "prd_has_masking": has_masking,
        "prd_sha256": sha,
    }
    overall = 0 if (completeness == 1.0 and compliance == 1.0) else 3
    print(f"[G1-PRD] 完整性={evidence['prd_completeness_pct']}%  合规性={evidence['prd_compliance_pct']}%  -> {'PASS' if overall==0 else 'FAIL'}")

    run_audit(ticket, prd, overall, evidence, args.actor)
    return overall


if __name__ == "__main__":
    raise SystemExit(main())
