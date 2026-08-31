#!/usr/bin/env python3
"""Static, text-only integrity checks for frontend build source inputs.

The gate never imports, requires, evaluates, or otherwise executes a target
file. It hashes raw file bytes, decodes build inputs as UTF-8 text, parses
package manifests with the Python standard library, masks JavaScript comments,
and applies narrowly scoped signatures to executable build entrypoints.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence


KNOWN_MALICIOUS_SHA256 = frozenset(
    {
        "556812c8ec8177751aa22b8fa641a92e782f9e2564866887061c6626186bd5f0",
    }
)

REQUIRED_SOURCE_INPUTS = (Path("pc/package.json"), Path("pc/postcss.config.js"))
EXECUTABLE_CONFIG_EXTENSIONS = frozenset({".js", ".cjs", ".mjs", ".ts", ".cts", ".mts"})
SPECIAL_BUILD_ENTRYPOINTS = frozenset(
    {
        "gulpfile.js",
        "gulpfile.cjs",
        "gulpfile.mjs",
        "gulpfile.ts",
        "gruntfile.js",
        "gruntfile.cjs",
        "gruntfile.mjs",
        "gruntfile.ts",
    }
)

# Generated output, dependencies, caches, and VCS metadata are outside the
# source-config trust boundary. This is not a content or rule allowlist.
EXCLUDED_DIRECTORY_NAMES = frozenset(
    {
        ".cache",
        ".git",
        ".hg",
        ".next",
        ".nuxt",
        ".output",
        ".svn",
        ".turbo",
        ".vite",
        "coverage",
        "dist",
        "node_modules",
        "out",
        "target",
        "vendor",
    }
)

MAX_SOURCE_INPUT_BYTES = 2_000_000
LONG_EXECUTABLE_LINE_LENGTH = 2_000
HIGH_ENTROPY_LINE_LENGTH = 1_200
HIGH_ENTROPY_THRESHOLD = 4.6
HIGH_ENTROPY_PUNCTUATION_RATIO = 0.10

_IMPORT_PREFIX = r"(?:\bfrom\s*|\brequire\s*\(\s*|\bimport\s*\(\s*|\bimport\s+)"
_NETWORK_MODULES = r"(?:http2?|https|net|tls|dns|dgram|undici|axios|node-fetch|got|superagent)"
_CHILD_PROCESS_MODULES = r"(?:child_process|execa|cross-spawn)"

DIRECT_NETWORK_MODULE_RE = re.compile(
    _IMPORT_PREFIX + rf"[\"'](?:node:)?{_NETWORK_MODULES}(?:/[^\"']*)?[\"']"
)
DIRECT_NETWORK_API_RE = re.compile(
    r"(?:\bfetch\s*\(|\bnew\s+XMLHttpRequest\s*\(|\bnew\s+WebSocket\s*\(|"
    r"\bDeno\.connect(?:Tls)?\s*\(|\bBun\.connect\s*\()"
)
DIRECT_CHILD_PROCESS_MODULE_RE = re.compile(
    _IMPORT_PREFIX + rf"[\"'](?:node:)?{_CHILD_PROCESS_MODULES}(?:/[^\"']*)?[\"']"
)
DIRECT_CHILD_PROCESS_API_RE = re.compile(
    r"(?:\bBun\.spawn(?:Sync)?\s*\(|\bnew\s+Deno\.Command\s*\(|"
    r"\bprocess\.binding\s*\(\s*[\"']spawn_sync[\"'])"
)
DYNAMIC_EVALUATION_RE = re.compile(
    r"(?:\beval\b|\b(?:new\s+)?Function\s*\(|"
    r"\bvm\.(?:runInContext|runInNewContext|runInThisContext|compileFunction)\s*\()"
)
NETWORK_CAPABILITY_RE = re.compile(
    r"\b(?:fetch|XMLHttpRequest|WebSocket|http|https|http2|net|tls|dns|dgram|"
    r"undici|axios|node-fetch|got|superagent)\b"
)
PROCESS_CAPABILITY_RE = re.compile(
    r"\b(?:process|child_process|exec|execFile|execSync|execFileSync|spawn|spawnSync|"
    r"fork|execa|cross-spawn|Bun|Deno)\b"
)
ENCODED_LOADER_RE = re.compile(
    r"(?:\bBuffer\.from\s*\(|\batob\s*\(|\bString\.fromCharCode\s*\()"
)
EXECUTABLE_LINE_RE = re.compile(
    r"(?:=>|\b(?:class|const|eval|export|function|Function|import|let|require|return|var)\b|"
    r"[{};])"
)

# npm executes these package-root hooks automatically during ``npm ci``. The
# release path intentionally permits none: a future legitimate hook must first
# receive an explicit, reviewed integrity policy instead of executing before
# the repository's build configuration is loaded.
AUTOMATIC_INSTALL_LIFECYCLE_SCRIPTS = frozenset(
    {
        "preinstall",
        "install",
        "postinstall",
        "prepublish",
        "preprepare",
        "prepare",
        "postprepare",
    }
)

# ``npm run EVENT`` also invokes preEVENT/postEVENT. These protected release
# events must remain direct, reviewable commands rather than hidden hooks.
PROTECTED_RELEASE_SCRIPT_EVENTS = frozenset(
    {
        "build",
        "build:pwa",
        "build:tauri",
        "lint",
        "test",
        "test:margin",
        "type-check",
        "typecheck",
    }
)

# The P0 gate promises to execute a real PC Vite/PostCSS production build.
# Pinning this entry prevents a manifest edit from silently replacing that
# build with a no-op or a different executable before review.
REQUIRED_PACKAGE_SCRIPTS = {
    Path("pc/package.json"): {"build": "vite build"},
}

PACKAGE_SCRIPT_NETWORK_RE = re.compile(
    r"(?:^|[\s;&|])(?:curl|wget|nc|ncat|telnet|ftp|sftp|scp|ssh)\b|"
    r"\b(?:Invoke-WebRequest|Invoke-RestMethod|Start-BitsTransfer)\b|"
    r"\bcertutil\b[^\n]*(?:-urlcache|-verifyctl)",
    re.IGNORECASE,
)
PACKAGE_SCRIPT_DYNAMIC_RE = re.compile(
    r"(?:^|[\s;&|])(?:node|deno|bun|python(?:3)?|perl|ruby|bash|sh|zsh|pwsh|powershell)"
    r"\s+(?:--eval|-e|-c|/c)\b|\beval\s+",
    re.IGNORECASE,
)
PACKAGE_SCRIPT_ENCODED_RE = re.compile(
    r"\b(?:base64\s+(?:--decode|-d)|openssl\s+(?:enc|base64)|xxd\s+-r|"
    r"certutil\s+-decode)\b",
    re.IGNORECASE,
)


@dataclass(frozen=True)
class Finding:
    path: str
    rule: str
    message: str
    line: int | None = None

    def render(self) -> str:
        location = f"{self.path}:{self.line}" if self.line is not None else self.path
        return f"{location} [{self.rule}] {self.message}"


@dataclass(frozen=True)
class ScanReport:
    files_scanned: int
    findings: tuple[Finding, ...]


def is_executable_build_config(path: Path) -> bool:
    """Return whether a path is an executable JS/TS build entrypoint."""

    name = path.name.lower()
    if path.suffix.lower() not in EXECUTABLE_CONFIG_EXTENSIONS:
        return False
    return (
        ".config." in name
        or name.startswith((".babelrc.", ".eslintrc.", ".postcssrc.", ".prettierrc."))
        or name in SPECIAL_BUILD_ENTRYPOINTS
    )


def _discover_source_inputs(root: Path) -> tuple[list[Path], list[Finding]]:
    paths: set[Path] = set()
    findings: list[Finding] = []

    def record_walk_error(error: OSError) -> None:
        filename = error.filename or str(root)
        try:
            display = str(Path(filename).relative_to(root))
        except ValueError:
            display = filename
        findings.append(
            Finding(
                path=display,
                rule="path-read-error",
                message=f"failed to enumerate build-config scope: {error.strerror or error}",
            )
        )

    for directory, directory_names, filenames in os.walk(root, topdown=True, onerror=record_walk_error):
        directory_names[:] = sorted(
            name for name in directory_names if name not in EXCLUDED_DIRECTORY_NAMES
        )
        base = Path(directory)
        for filename in sorted(filenames):
            path = base / filename
            if filename == "package.json" or is_executable_build_config(path):
                paths.add(path)

    for relative_path in REQUIRED_SOURCE_INPUTS:
        path = root / relative_path
        if not path.exists() and not path.is_symlink():
            findings.append(
                Finding(
                    path=relative_path.as_posix(),
                    rule="required-source-input-missing",
                    message="required build source input is missing; integrity cannot be established",
                )
            )
        else:
            paths.add(path)

    return sorted(paths, key=lambda path: path.relative_to(root).as_posix()), findings


def _mask_javascript_comments(text: str) -> str:
    """Replace JS comments with spaces while preserving strings and newlines."""

    output: list[str] = []
    index = 0
    state = "normal"
    quote = ""

    while index < len(text):
        char = text[index]
        following = text[index + 1] if index + 1 < len(text) else ""

        if state == "normal":
            if char in {"'", '"', "`"}:
                state = "string"
                quote = char
                output.append(char)
                index += 1
            elif char == "/" and following == "/":
                state = "line-comment"
                output.extend((" ", " "))
                index += 2
            elif char == "/" and following == "*":
                state = "block-comment"
                output.extend((" ", " "))
                index += 2
            else:
                output.append(char)
                index += 1
            continue

        if state == "string":
            output.append(char)
            if char == "\\" and index + 1 < len(text):
                output.append(text[index + 1])
                index += 2
            else:
                if char == quote:
                    state = "normal"
                index += 1
            continue

        if state == "line-comment":
            if char in {"\n", "\r"}:
                output.append(char)
                state = "normal"
            else:
                output.append(" ")
            index += 1
            continue

        if state == "block-comment":
            if char == "*" and following == "/":
                output.extend((" ", " "))
                index += 2
                state = "normal"
            else:
                output.append(char if char in {"\n", "\r"} else " ")
                index += 1

    return "".join(output)


def _mask_javascript_strings(text: str) -> str:
    """Mask inert JS string contents while preserving code offsets/newlines."""

    output: list[str] = []
    index = 0
    quote = ""

    while index < len(text):
        char = text[index]
        if not quote:
            if char in {"'", '"', "`"}:
                quote = char
                output.append(" ")
            else:
                output.append(char)
            index += 1
            continue

        if char == "\\" and index + 1 < len(text):
            output.append(" ")
            escaped = text[index + 1]
            output.append(escaped if escaped in {"\n", "\r"} else " ")
            index += 2
            continue

        output.append(char if char in {"\n", "\r"} else " ")
        if char == quote:
            quote = ""
        index += 1

    return "".join(output)


def _line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def _shannon_entropy(text: str) -> float:
    if not text:
        return 0.0
    length = len(text)
    return -sum((count / length) * math.log2(count / length) for count in Counter(text).values())


def _is_suspicious_long_line(line: str) -> bool:
    stripped = line.strip()
    if len(stripped) < HIGH_ENTROPY_LINE_LENGTH or not EXECUTABLE_LINE_RE.search(stripped):
        return False
    if len(stripped) >= LONG_EXECUTABLE_LINE_LENGTH:
        return True
    punctuation = sum(not char.isalnum() and not char.isspace() for char in stripped)
    return (
        punctuation / len(stripped) >= HIGH_ENTROPY_PUNCTUATION_RATIO
        and _shannon_entropy(stripped) >= HIGH_ENTROPY_THRESHOLD
    )


def _earliest_match(matches: Iterable[re.Match[str] | None]) -> re.Match[str] | None:
    present_matches = (match for match in matches if match is not None)
    return min(present_matches, key=lambda match: match.start(), default=None)


def _first_match(patterns: Iterable[re.Pattern[str]], text: str) -> re.Match[str] | None:
    return _earliest_match(pattern.search(text) for pattern in patterns)


def _first_structural_match(
    pattern: re.Pattern[str],
    text_with_strings: str,
    code_only_text: str,
) -> re.Match[str] | None:
    return next(
        (
            match
            for match in pattern.finditer(text_with_strings)
            if code_only_text[match.start() : match.end()].strip()
        ),
        None,
    )


def _inspect_config(relative_path: str, text: str) -> list[Finding]:
    findings: list[Finding] = []
    masked = _mask_javascript_comments(text)
    code_only = _mask_javascript_strings(masked)

    for line_number, line in enumerate(masked.splitlines(), 1):
        if _is_suspicious_long_line(line):
            findings.append(
                Finding(
                    path=relative_path,
                    line=line_number,
                    rule="long-executable-line",
                    message="build config contains a long or high-entropy executable line",
                )
            )

    network_match = _earliest_match(
        (
            _first_structural_match(DIRECT_NETWORK_MODULE_RE, masked, code_only),
            DIRECT_NETWORK_API_RE.search(code_only),
        )
    )
    if network_match is not None:
        findings.append(
            Finding(
                path=relative_path,
                line=_line_number(masked, network_match.start()),
                rule="direct-network-capability",
                message="executable build config directly accesses a network capability",
            )
        )

    child_process_match = _earliest_match(
        (
            _first_structural_match(DIRECT_CHILD_PROCESS_MODULE_RE, masked, code_only),
            DIRECT_CHILD_PROCESS_API_RE.search(code_only),
        )
    )
    if child_process_match is not None:
        findings.append(
            Finding(
                path=relative_path,
                line=_line_number(masked, child_process_match.start()),
                rule="direct-child-process-capability",
                message="executable build config directly accesses a child-process capability",
            )
        )

    dynamic_match = DYNAMIC_EVALUATION_RE.search(code_only)
    runtime_match = _earliest_match(
        (
            _first_match((NETWORK_CAPABILITY_RE, PROCESS_CAPABILITY_RE), code_only),
            network_match,
            child_process_match,
        )
    )
    if dynamic_match is not None and runtime_match is not None:
        findings.append(
            Finding(
                path=relative_path,
                line=_line_number(masked, dynamic_match.start()),
                rule="dynamic-eval-runtime-capability",
                message="dynamic evaluation is combined with network or process capability",
            )
        )

    encoded_match = ENCODED_LOADER_RE.search(code_only)
    if encoded_match is not None and dynamic_match is not None:
        findings.append(
            Finding(
                path=relative_path,
                line=_line_number(masked, encoded_match.start()),
                rule="encoded-dynamic-loader",
                message="encoded data handling is combined with dynamic evaluation in build config",
            )
        )

    return findings


def _inspect_package_manifest(relative_path: str, text: str) -> list[Finding]:
    """Inspect package scripts without invoking npm or a JavaScript runtime."""

    try:
        manifest = json.loads(text)
    except json.JSONDecodeError as error:
        return [
            Finding(
                path=relative_path,
                line=error.lineno,
                rule="invalid-package-manifest",
                message=f"package.json is not valid JSON: {error.msg}",
            )
        ]

    if not isinstance(manifest, dict):
        return [
            Finding(
                path=relative_path,
                rule="invalid-package-manifest",
                message="package.json root must be a JSON object",
            )
        ]

    scripts = manifest.get("scripts", {})
    if not isinstance(scripts, dict):
        return [
            Finding(
                path=relative_path,
                rule="invalid-package-scripts",
                message="package.json scripts must be a JSON object",
            )
        ]

    findings: list[Finding] = []
    for script_name, command in sorted(scripts.items()):
        if not isinstance(script_name, str) or not isinstance(command, str):
            findings.append(
                Finding(
                    path=relative_path,
                    rule="invalid-package-script",
                    message="package.json script names and commands must be strings",
                )
            )
            continue

        normalized_name = script_name.strip().lower()
        normalized_command = command.strip()
        if not normalized_command:
            findings.append(
                Finding(
                    path=relative_path,
                    rule="blank-package-script",
                    message=f"package script {script_name!r} is blank",
                )
            )
            continue

        if normalized_name in AUTOMATIC_INSTALL_LIFECYCLE_SCRIPTS:
            findings.append(
                Finding(
                    path=relative_path,
                    rule="automatic-install-lifecycle-script",
                    message=(
                        f"package script {script_name!r} executes automatically during npm install/ci"
                    ),
                )
            )

        for event in PROTECTED_RELEASE_SCRIPT_EVENTS:
            if normalized_name in {f"pre{event}", f"post{event}"}:
                findings.append(
                    Finding(
                        path=relative_path,
                        rule="protected-release-lifecycle-hook",
                        message=(
                            f"package script {script_name!r} hides execution around protected "
                            f"release event {event!r}"
                        ),
                    )
                )
                break

        if len(normalized_command) >= LONG_EXECUTABLE_LINE_LENGTH or _is_suspicious_long_line(
            normalized_command
        ):
            findings.append(
                Finding(
                    path=relative_path,
                    rule="long-package-script",
                    message=f"package script {script_name!r} is long or high-entropy",
                )
            )

        if PACKAGE_SCRIPT_NETWORK_RE.search(normalized_command):
            findings.append(
                Finding(
                    path=relative_path,
                    rule="package-script-network-capability",
                    message=f"package script {script_name!r} directly accesses the network",
                )
            )

        if PACKAGE_SCRIPT_DYNAMIC_RE.search(normalized_command):
            findings.append(
                Finding(
                    path=relative_path,
                    rule="package-script-dynamic-evaluation",
                    message=f"package script {script_name!r} dynamically evaluates inline code",
                )
            )

        if PACKAGE_SCRIPT_ENCODED_RE.search(normalized_command):
            findings.append(
                Finding(
                    path=relative_path,
                    rule="package-script-encoded-loader",
                    message=f"package script {script_name!r} decodes executable content",
                )
            )

    required_scripts = REQUIRED_PACKAGE_SCRIPTS.get(Path(relative_path), {})
    for script_name, expected_command in required_scripts.items():
        actual_command = scripts.get(script_name)
        if actual_command != expected_command:
            findings.append(
                Finding(
                    path=relative_path,
                    rule="required-package-script-mismatch",
                    message=(
                        f"package script {script_name!r} must remain {expected_command!r}; "
                        "review and update the integrity policy before changing it"
                    ),
                )
            )

    return findings


def scan_repository(root: Path) -> ScanReport:
    """Scan build inputs below ``root`` without importing or executing them."""

    root = root.resolve()
    if not root.is_dir():
        return ScanReport(
            files_scanned=0,
            findings=(
                Finding(
                    path=str(root),
                    rule="scan-root-invalid",
                    message="scan root is not an existing directory",
                ),
            ),
        )

    paths, findings = _discover_source_inputs(root)
    files_scanned = 0

    for path in paths:
        relative_path = path.relative_to(root).as_posix()
        if path.is_symlink():
            findings.append(
                Finding(
                    path=relative_path,
                    rule="source-input-symlink",
                    message="build source inputs must be regular repository files",
                )
            )
            continue

        try:
            raw = path.read_bytes()
        except OSError as error:
            findings.append(
                Finding(
                    path=relative_path,
                    rule="source-input-read-error",
                    message=f"failed to read build source input: {error.strerror or error}",
                )
            )
            continue

        files_scanned += 1
        digest = hashlib.sha256(raw).hexdigest()
        if digest in KNOWN_MALICIOUS_SHA256:
            findings.append(
                Finding(
                    path=relative_path,
                    rule="known-malicious-sha256",
                    message=f"file matches blocked SHA-256 {digest}",
                )
            )

        if len(raw) > MAX_SOURCE_INPUT_BYTES:
            findings.append(
                Finding(
                    path=relative_path,
                    rule="oversized-source-input",
                    message=f"build source input exceeds the {MAX_SOURCE_INPUT_BYTES}-byte inspection limit",
                )
            )
            continue

        try:
            text = raw.decode("utf-8")
        except UnicodeDecodeError as error:
            findings.append(
                Finding(
                    path=relative_path,
                    rule="non-utf8-source-input",
                    message=f"build source input is not valid UTF-8 text at byte {error.start}",
                )
            )
            continue

        if path.name == "package.json":
            findings.extend(_inspect_package_manifest(relative_path, text))
        else:
            findings.extend(_inspect_config(relative_path, text))

    ordered_findings = tuple(
        sorted(
            findings,
            key=lambda finding: (
                finding.path,
                finding.line if finding.line is not None else 0,
                finding.rule,
                finding.message,
            ),
        )
    )
    return ScanReport(files_scanned=files_scanned, findings=ordered_findings)


def _build_argument_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Statically scan frontend build inputs for source-integrity risks."
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="repository root (defaults to the parent of scripts/)",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _build_argument_parser().parse_args(argv)
    report = scan_repository(args.root)
    if report.findings:
        print(
            f"Source integrity gate failed: {len(report.findings)} finding(s) "
            f"across {report.files_scanned} scanned build source input(s).",
            file=sys.stderr,
        )
        for finding in report.findings:
            print(f"- {finding.render()}", file=sys.stderr)
        return 1

    print(
        f"Source integrity gate passed: {report.files_scanned} build source input(s) "
        "scanned as UTF-8 text."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
