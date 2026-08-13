#!/usr/bin/env python3
"""检查后端中文 doc 注释的质量退化信号：同文件重复粘贴、纯复述函数名、空洞套话。"""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path

CHINESE = re.compile(r"[\u3400-\u9fff]")
FN_RE = re.compile(r"^\s*(pub(?:\([^)]*\))?\s+)?(async\s+)?(unsafe\s+)?fn\s+([A-Za-z0-9_]+)")
DOC_RE = re.compile(r"^\s*///(.*)$")
ATTR_RE = re.compile(r"^\s*#\[")

HOLLOW_PATTERNS = [
    re.compile(r"^查询数据库[并返回结果]*。?$"),
    re.compile(r"^处理请求。?$"),
    re.compile(r"^返回结果。?$"),
    re.compile(r"^执行操作。?$"),
    re.compile(r"^辅助函数。?$"),
]


def collect(path: Path):
    lines = path.read_text(encoding="utf-8").splitlines()
    entries = []
    doc_buffer: list[str] = []
    for index, line in enumerate(lines):
        doc_match = DOC_RE.match(line)
        if doc_match:
            doc_buffer.append(doc_match.group(1).strip())
            continue
        if ATTR_RE.match(line):
            continue
        fn_match = FN_RE.match(line)
        if fn_match:
            entries.append(
                {
                    "line": index + 1,
                    "name": fn_match.group(4),
                    "doc": "\n".join(doc_buffer).strip(),
                }
            )
        if line.strip():
            doc_buffer = []
    return entries


def main() -> int:
    duplicates: list[str] = []
    hollow: list[str] = []
    total_documented = 0

    for path in sorted(Path("src").rglob("*.rs")):
        entries = collect(path)
        by_doc = defaultdict(list)
        for entry in entries:
            doc = entry["doc"]
            if not doc or not CHINESE.search(doc):
                continue
            total_documented += 1
            # 只有具备实质长度的注释被复用才算退化，一行短说明允许在同类小函数间共享。
            if len(CHINESE.findall(doc)) >= 20:
                by_doc[doc].append(entry)
            for pattern in HOLLOW_PATTERNS:
                if pattern.match(doc):
                    hollow.append(f"{path}:{entry['line']} `{entry['name']}` -> {doc[:60]}")

        for doc, group in by_doc.items():
            if len(group) >= 3:
                names = ", ".join(item["name"] for item in group[:5])
                duplicates.append(f"{path}: {len(group)} 个函数复用同一段 doc ({names}) -> {doc[:70]}")

    print(f"已有中文注释的函数数: {total_documented}")
    print(f"\n== 同文件重复粘贴 (>=3 处复用同一段 >=20 字注释) ==  {len(duplicates)} 处")
    for item in duplicates[:30]:
        print(f"  {item}")
    if len(duplicates) > 30:
        print(f"  ... 另有 {len(duplicates) - 30} 处")

    print(f"\n== 空洞套话 ==  {len(hollow)} 处")
    for item in hollow[:20]:
        print(f"  {item}")

    return 1 if duplicates or hollow else 0


if __name__ == "__main__":
    sys.exit(main())
