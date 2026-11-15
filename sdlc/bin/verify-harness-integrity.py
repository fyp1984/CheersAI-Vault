#!/usr/bin/env python3
"""
verify-harness-integrity.py — HE-2 (Deterministic) + HE-5 (Harness-as-Code)
------------------------------------------------------------
计算所有 Harness 文件（bin/* + gates/* + harnesses/* + policies/* + templates/*）
的 sha256，并与 `sdlc/harnesses/_harness-integrity.manifest.json` 比对。

三种模式（严格度递减）：
  1) --strict    (默认)：任何 mismatch → exit 非 0 立即阻断 （HE-4 Zero-Trusted）
  2) --warn-only：mismatch 仅 WARN，退出码仍为 0（允许首次演示 / CI 未落地双签前的过渡）
  3) --regen-hashes：写 manifest （需 TD+RO 双签 PR 后才能合入，README §3 流程）
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

SDLC_ROOT = Path(__file__).resolve().parent.parent
MANIFEST_PATH = SDLC_ROOT / "harnesses" / "_harness-integrity.manifest.json"

# HE-5：Harness-as-Code 范围（本脚本自身不计入 → 避免鸡生蛋）
SCAN_ROOTS = [
    ("bin",      SDLC_ROOT / "bin",      True,  ["*.py", "*.sh"], True),   # recursive=True
    ("gates",    SDLC_ROOT / "gates",    False, ["*.yaml"], False),
    ("harnesses",SDLC_ROOT / "harnesses",False, ["G*.harness.yaml"], False),  # 不含 _manifest.json itself (在 scan 后过滤)
    ("policies", SDLC_ROOT / "policies", False, ["*.yaml", "*.json"], False),
    ("templates",SDLC_ROOT / "templates",False, ["*.md"], False),
]

SELF_NAME = Path(__file__).name


def sha256_file(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def collect_all() -> dict[str, str]:
    out: dict[str, str] = {}
    for tag, root, recursive, patterns, _rec in SCAN_ROOTS:
        if not root.exists():
            continue
        for pat in patterns:
            it = root.rglob(pat) if recursive else root.glob(pat)
            for f in sorted(it):
                if not f.is_file():
                    continue
                if f.name == SELF_NAME:
                    continue
                if f.name.startswith("_harness-integrity.manifest"):
                    continue
                rel = f.relative_to(SDLC_ROOT).as_posix()
                out[rel] = sha256_file(f)
    return out


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--regen-hashes", action="store_true",
                   help="[HE-5] 只有 TD+RO 双签 PR 合并后才能调用，写入 manifest")
    p.add_argument("--strict", action="store_true",
                   help="[HE-4] 严格模式，任何缺失/mismatch → FAIL（与 --warn-only 互斥，默认行为由 --warn-only 决定）")
    p.add_argument("--warn-only", action="store_true",
                   help="[演示/过渡模式] 有 hash 不一致仅打印 WARNING，exit=0（待 TD+RO 双签后关闭）")
    args = p.parse_args()

    current = collect_all()
    if not MANIFEST_PATH.exists():
        if not args.regen_hashes:
            print(f"[verify-harness-integrity] ❌ manifest 不存在: {MANIFEST_PATH}")
            print("   首次使用请执行: python3 sdlc/bin/verify-harness-integrity.py --regen-hashes")
            print("   并提交 PR 经 TD+RO 双签后合入。(HE-5)")
            return 9
        MANIFEST_PATH.parent.mkdir(parents=True, exist_ok=True)
        MANIFEST_PATH.write_text(
            json.dumps({"files": current, "generated_by": SELF_NAME},
                       ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        print(f"[verify-harness-integrity] ✅ manifest 已生成 -> {MANIFEST_PATH}")
        print(f"   共 {len(current)} 个文件已 hash 锁定。请提交 PR → TD+RO 双签 (HE-5)")
        return 0

    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    expected: dict[str, str] = manifest.get("files", {})

    missing = [f for f in expected if f not in current]
    extra = [f for f in current if f not in expected]
    mismatches = [(f, expected[f], current[f])
                  for f in current if f in expected and expected[f] != current[f]]

    total_issues = len(missing) + len(extra) + len(mismatches)

    if args.regen_hashes:
        MANIFEST_PATH.write_text(
            json.dumps({"files": current, "regenerated_by": SELF_NAME},
                       ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        print(f"[verify-harness-integrity] 🔁 manifest 已重写（需走 HE-5 双签 PR 流程）: {total_issues} 处变化")
        if missing:    print(f"   删除条目: {missing[:10]}{'…' if len(missing)>10 else ''}")
        if extra:      print(f"   新增条目: {extra[:10]}{'…' if len(extra)>10 else ''}")
        if mismatches: print(f"   变更条目: {[m[0] for m in mismatches[:10]]}{'…' if len(mismatches)>10 else ''}")
        return 0

    # 正常校验 (run-pipeline.sh 启动模式)
    strict_mode = (not args.warn_only)
    print(f"[verify-harness-integrity] 🔍 正在校验 {len(current)} 个 Harness-as-Code 文件 hash… (strict={strict_mode})")

    # HE-5 特判：若 manifest 仍是 SEED 占位 → 视为 TD+RO 尚未双签落地 → 自动降级 --warn-only
    seed_markers = [v for v in expected.values() if isinstance(v, str) and v.startswith("SEED-SHA256-")]
    if seed_markers:
        seed_files = [f for f, v in expected.items() if isinstance(v, str) and v.startswith("SEED-SHA256-")]
        print(f"[verify-harness-integrity] ℹ️  检测到 manifest 中 {len(seed_files)} 条仍为 SEED 占位（待 TD+RO 双签后 --regen-hashes 真 hash 覆盖），本次自动降级 --warn-only。")
        strict_mode = False

    if total_issues == 0:
        print(f"[verify-harness-integrity] ✅ Harness integrity PASS（HE-2 可复现 + HE-5 版本化）。")
        return 0

    print("[verify-harness-integrity] %s Harness integrity CHECK — 差异数=%d：" % (
          "⚠️ WARNING" if not strict_mode else "❌ FAIL (HE-4 Zero-Trusted)", total_issues))
    if missing:    print(f"   缺失文件: {missing}")
    if extra:      print(f"   未登记新增文件: {extra}")
    if mismatches: print(f"   hash 不一致 (manifest vs 实际)：")
    for f, e, c in mismatches:
        print(f"     - {f}: expected={e[:12]}…  actual={c[:12]}…")
    print("   👉 正确流程：HE-5 Harness-as-Code 变更需提交 PR → TD+RO 双签 → 合入后执行 --regen-hashes 重新锁 hash。")
    return 9 if strict_mode else 0


if __name__ == "__main__":
    raise SystemExit(main())
