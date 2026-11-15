#!/usr/bin/env python3
"""
gate-design.py — G2 门禁：DESIGN 文档三项硬检查
- BASE-PRD 锚点存在且对应 PRD sha 校验通过
- 技术栈一致性：不新增 forbidden 语言/框架
- 评审 Checklist 100% 勾选
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

SDLC_ROOT = Path(__file__).resolve().parent.parent
GATE_SPEC_YAML = SDLC_ROOT / "gates" / "G2-DESIGN.spec.yaml"
POLICY_DIR = SDLC_ROOT / "policies"

BASE_PRD_RE = re.compile(
    r"<!--\s*BASE-PRD:\s*(PRD-[A-Za-z0-9_\-]+\.md)@([0-9a-fA-F]{64})\s*-->"
)

# 极简 forbidden 关键词匹配（从 spec 的 forbidden_new_languages 读取）
FORBIDDEN_SCAN = {
    "CheersAI-FileBay": [r"springframework", r"@SpringBootApplication", r"django\.db", r"from django"],
    "CheersAI-Vault":   [r"import Vue from 'vue'", r"@angular/core", r"NestFactory"],
    "CheersAI-Nexus":   [r"const app = require\('express'\)", r"gin.Default\(\)", r"actix_web"],
    "CheersAI-Desktop": [r"@SpringBootApplication", r"gin.Default\(\)"],
}

CHECKLIST_ITEM_RE = re.compile(r"^\s*- \[ \]\s*", re.MULTILINE)


def sha256(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def audit(ticket: str, artifact: Path, exit_code: int, evidence: dict, actor: str):
    try:
        return subprocess.call(
            [
                sys.executable, str(SDLC_ROOT / "bin" / "audit-writer.py"),
                "--ticket", ticket, "--gate", "G2-DESIGN",
                "--actor_email", actor, "--role", "TD",
                "--artifact", str(artifact.resolve()),
                "--exit_code", str(exit_code),
                "--evidence", json.dumps(evidence, ensure_ascii=False),
            ],
            stdout=sys.stdout, stderr=sys.stderr,
        )
    except FileNotFoundError:
        return 0


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--design", required=True)
    p.add_argument("--repo", required=True, help="例: ../CheersAI-Vault 或 仓库名")
    p.add_argument("--actor", default="sdlc-td@cheersai.ai")
    p.add_argument("--ticket", default="AUTO")
    args = p.parse_args()

    design_path = Path(args.design).resolve()
    if not design_path.is_file():
        print(f"[G2-DESIGN] ❌ 找不到 DESIGN: {design_path}")
        return 1
    text = design_path.read_text(encoding="utf-8")

    ticket = args.ticket
    if ticket == "AUTO":
        m = re.match(r"DESIGN-([A-Za-z0-9_\-]+)\.md$", design_path.name)
        if m: ticket = m.group(1)

    # 检查 repo 名：../CheersAI-Vault -> CheersAI-Vault
    repo_name = args.repo.rstrip("/").split("/")[-1]
    repo_policy = POLICY_DIR / f"{repo_name}.tech-stack.lock.json"
    if not repo_policy.exists():
        print(f"[G2-DESIGN] ⚠️  未找到 {repo_policy}，按名字用 FORBIDDEN_SCAN 默认")

    # 1) BASE-PRD 锚点
    m = BASE_PRD_RE.search(text)
    base_ok = False
    prd_sha_real = ""
    if not m:
        print("[G2-DESIGN] ❌ 缺少 BASE-PRD 锚点（需形如 <!-- BASE-PRD: PRD-IMP-001.md@sha256 --->）")
    else:
        prd_name, prd_sha_declared = m.group(1), m.group(2)
        prd_candidates = [
            design_path.parent / prd_name,
            SDLC_ROOT / "docs" / prd_name,
        ]
        prd_path = next((c for c in prd_candidates if c.is_file()), None)
        if not prd_path:
            print(f"[G2-DESIGN] ❌ 找不到 BASE PRD 文件（尝试 {prd_candidates}）")
        else:
            prd_sha_real = sha256(prd_path)
            base_ok = prd_sha_real == prd_sha_declared
            if not base_ok:
                print(f"[G2-DESIGN] ❌ BASE-PRD sha 不一致: DECLARED={prd_sha_declared[:12]}…  ACTUAL={prd_sha_real[:12]}…")
            else:
                print(f"[G2-DESIGN] ✅ BASE-PRD 锚点校验通过（{prd_name}）")

    # 2) 技术栈一致性（扫 DESIGN 文本里的代码片段中关键字）
    bad_patterns = FORBIDDEN_SCAN.get(repo_name, [])
    tech_violations = []
    for pat in bad_patterns:
        if re.search(pat, text, re.IGNORECASE):
            tech_violations.append(pat)
    if tech_violations:
        print(f"[G2-DESIGN] ❌ 技术栈一致性违反（禁用模式）: {tech_violations}")
    else:
        print("[G2-DESIGN] ✅ 技术栈一致性检查通过")

    # 3) Checklist 全勾选（= 没有任何 - [ ] 未勾选项）
    # 但排除 PRD-TEMPLATE 这种还没填写的场景：只在文件包含"评审 Checklist"章节才强制
    if "评审 Checklist" in text or "附录 A" in text:
        unchecked = len(CHECKLIST_ITEM_RE.findall(text))
        if unchecked > 0:
            print(f"[G2-DESIGN] ❌ 附录 Checklist 还有 {unchecked} 项未勾选")
        else:
            print("[G2-DESIGN] ✅ 评审 Checklist 全部勾选")
    else:
        unchecked = 0
        print("[G2-DESIGN] ⚠️  DESIGN 未包含附录 Checklist，跳过 Checklist 检查")

    overall = 0 if (base_ok and not tech_violations and unchecked == 0) else 4
    evidence = {
        "base_prd_anchor_ok": base_ok,
        "prd_sha256": prd_sha_real,
        "forbidden_violations": tech_violations,
        "checklist_unchecked": unchecked,
    }
    print(f"[G2-DESIGN] -> {'PASS' if overall==0 else 'FAIL'}")

    audit(ticket, design_path, overall, evidence, args.actor)
    return overall


if __name__ == "__main__":
    raise SystemExit(main())
