#!/usr/bin/env python3
"""统计后端 Rust 源码中文 doc 注释覆盖率与字符量分布，用于注释丰富化任务分批。"""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path

CHINESE = re.compile(r"[\u3400-\u9fff]")
FN_RE = re.compile(r"^\s*(pub(?:\([^)]*\))?\s+)?(async\s+)?(unsafe\s+)?fn\s+([A-Za-z0-9_]+)")
DOC_RE = re.compile(r"^\s*///(.*)$")
ATTR_RE = re.compile(r"^\s*#\[")


def analyze(path: Path):
    lines = path.read_text(encoding="utf-8").splitlines()
    results = []
    doc_buffer: list[str] = []
    for index, line in enumerate(lines):
        doc_match = DOC_RE.match(line)
        if doc_match:
            doc_buffer.append(doc_match.group(1))
            continue
        if ATTR_RE.match(line):
            continue
        fn_match = FN_RE.match(line)
        if fn_match:
            doc_text = "\n".join(doc_buffer)
            results.append(
                {
                    "line": index + 1,
                    "name": fn_match.group(4),
                    "is_pub": bool(fn_match.group(1)),
                    "doc_lines": len(doc_buffer),
                    "cjk": len(CHINESE.findall(doc_text)),
                }
            )
        if line.strip():
            doc_buffer = []
    return results


def main() -> int:
    root = Path("src")
    per_file = defaultdict(lambda: {"total": 0, "undoc": 0, "thin": 0, "cjk": 0, "pub": 0})
    buckets = defaultdict(int)
    grand_total = 0

    for path in sorted(root.rglob("*.rs")):
        rel = str(path)
        for fn in analyze(path):
            grand_total += 1
            stat = per_file[rel]
            stat["total"] += 1
            stat["cjk"] += fn["cjk"]
            if fn["is_pub"]:
                stat["pub"] += 1
            if fn["cjk"] == 0:
                stat["undoc"] += 1
                buckets["0 (无中文注释)"] += 1
            elif fn["cjk"] < 40:
                stat["thin"] += 1
                buckets["1-39 (过于简单)"] += 1
            elif fn["cjk"] < 80:
                buckets["40-79 (基本合格)"] += 1
            elif fn["cjk"] < 150:
                buckets["80-149 (较丰富)"] += 1
            else:
                buckets["150+ (充分)"] += 1

    print(f"函数总数: {grand_total}")
    print("\n== 中文注释字符数分布 ==")
    for key in sorted(buckets):
        count = buckets[key]
        print(f"  {key:22s} {count:5d}  ({count * 100 / grand_total:5.1f}%)")

    print("\n== 待改进文件全量清单 (无注释 + 过简) ==")
    ranked = sorted(
        per_file.items(),
        key=lambda item: item[1]["undoc"] + item[1]["thin"],
        reverse=True,
    )
    limit = int(sys.argv[1]) if len(sys.argv) > 1 else 45
    shown = 0
    for rel, stat in ranked:
        need = stat["undoc"] + stat["thin"]
        if need == 0:
            break
        shown += 1
        if shown > limit:
            continue
        print(
            f"  {need:4d} 待补 (无{stat['undoc']:3d} 简{stat['thin']:3d}) / 共{stat['total']:4d}  {rel}"
        )
    print(f"  ... 共 {shown} 个文件需要改进")

    total_need = sum(stat["undoc"] + stat["thin"] for stat in per_file.values())
    print(f"\n合计待改进函数: {total_need}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
