#!/usr/bin/env python3
"""
audit-writer.py — Harness Engineering HE-3 (Observability First 观测优先) + HE-2 (Immutable Audit Chain)
-----------------------------------------------
把门禁结果写入 sdlc/audit/YYYYMMDD/_<session>.jsonl，并做链式哈希：
  record["prev_hash"] = sha256(上一条记录的完整 JSON)

用法:
  ./bin/audit-writer.py \
      --ticket IMP-001 --gate G1-PRD \
      --actor_email sdlc-ra@cheersai.ai --role RA \
      --artifact ../sdlc/docs/PRD-IMP-001.md \
      --exit_code 0 --evidence '{"prd_completeness":100,"prd_compliance":100}'
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import time
from pathlib import Path

SDLC_ROOT = Path(__file__).resolve().parent.parent
AUDIT_ROOT = SDLC_ROOT / "audit"


def sha256_bytes(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def today_dir() -> Path:
    d = AUDIT_ROOT / time.strftime("%Y%m%d", time.localtime())
    d.mkdir(parents=True, exist_ok=True)
    return d


def latest_session_file(date_dir: Path) -> Path | None:
    files = sorted(date_dir.glob("_*.jsonl"))
    return files[-1] if files else None


def load_prev_hash(f: Path) -> str:
    if not f.exists() or f.stat().st_size == 0:
        return "0" * 64
    last_line = ""
    with f.open("rb") as fh:
        fh.seek(0, 2)
        size = fh.tell()
        buf = bytearray()
        pos = size - 1
        while pos >= 0 and len(buf) < 2 ** 20:  # 最大 1MB 兜底
            fh.seek(pos)
            ch = fh.read(1)
            if ch == b"\n" and buf:
                break
            buf[0:0] = ch
            pos -= 1
        last_line = buf.decode("utf-8", "replace").strip() or ""
    if not last_line:
        return "0" * 64
    return sha256_bytes(last_line.encode("utf-8"))


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--ticket", required=True)
    p.add_argument("--gate", required=True)
    p.add_argument("--actor_email", required=True)
    p.add_argument("--role", choices=["RA", "TD", "CD", "QA", "RO", "SYSTEM"], required=True)
    p.add_argument("--artifact", default=None, help="交付物文件路径，用于计算 artifact_sha256")
    p.add_argument("--exit_code", type=int, required=True)
    p.add_argument("--evidence", type=str, default="{}", help="JSON str")
    args = p.parse_args()

    d = today_dir()
    session_id = os.environ.get("SDLC_SESSION_ID", f"session-{int(time.time())}")
    session_file = d / f"_{session_id}.jsonl"

    # 交付物 sha256
    artifact_sha = "0" * 64
    if args.artifact:
        ap = Path(args.artifact).resolve()
        if ap.is_file():
            artifact_sha = sha256_bytes(ap.read_bytes())

    evidence_obj: dict = {}
    try:
        evidence_obj = json.loads(args.evidence) if args.evidence else {}
    except json.JSONDecodeError as e:
        print(f"[audit-writer] evidence JSON parse error: {e}", file=sys.stderr)

    record = {
        "ticket": args.ticket,
        "gate": args.gate,
        "ts_unix": int(time.time()),
        "actor_email": args.actor_email,
        "role": args.role,
        "artifact_sha256": artifact_sha,
        "exit_code": args.exit_code,
        "evidence": evidence_obj,
        "prev_hash": load_prev_hash(session_file),
    }
    line = json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n"
    with session_file.open("a", encoding="utf-8") as f:
        f.write(line)
    # stdout 给下游读
    print(f"[audit-writer] ok -> {session_file}")
    print(f"[audit-writer] record_prev_hash={record['prev_hash']}")
    print(f"[audit-writer] artifact_sha256={artifact_sha}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
