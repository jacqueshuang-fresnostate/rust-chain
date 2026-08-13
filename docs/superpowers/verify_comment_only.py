#!/usr/bin/env python3
"""校验 src/ 下的工作区改动是否只涉及注释行，防止注释任务误改可执行代码。

用法：python3 docs/superpowers/verify_comment_only.py
退出码 0 表示全部改动均为注释；非 0 表示存在疑似代码改动，需人工复核。
"""

from __future__ import annotations

import re
import subprocess
import sys

COMMENT_PREFIX = re.compile(r"^\s*(///|//!|//)")


def changed_rust_files() -> list[str]:
    output = subprocess.run(
        ["git", "diff", "--name-only", "--", "src/"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return [line for line in output.splitlines() if line.endswith(".rs")]


def main() -> int:
    files = changed_rust_files()
    if not files:
        print("src/ 下没有改动。")
        return 0

    diff = subprocess.run(
        ["git", "diff", "--unified=0", "--", "src/"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout

    current_file = ""
    suspicious: list[tuple[str, str]] = []
    for line in diff.splitlines():
        if line.startswith("+++ b/"):
            current_file = line[6:]
            continue
        if line.startswith(("+++", "---", "@@", "diff ", "index ", "new file", "deleted file")):
            continue
        if not line.startswith(("+", "-")):
            continue

        body = line[1:]
        # 空行增删属于注释块排版，不视为代码改动。
        if not body.strip():
            continue
        if COMMENT_PREFIX.match(body):
            continue
        suspicious.append((current_file, line))

    print(f"改动的 Rust 文件数: {len(files)}")
    if suspicious:
        print(f"\n发现 {len(suspicious)} 行疑似非注释改动，需人工复核：\n")
        for path, line in suspicious[:80]:
            print(f"  {path}: {line[:150]}")
        if len(suspicious) > 80:
            print(f"  ... 另有 {len(suspicious) - 80} 行")
        return 1

    print("全部改动均为注释行，未触碰可执行代码。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
