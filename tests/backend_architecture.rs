use std::{collections::BTreeSet, fs, path::Path};

const DDD_LAYERS: &[&str] = &[
    "domain",
    "repository",
    "service",
    "application",
    "infrastructure",
    "presentation",
];

#[test]
/// DDD 层按职责可选；但一旦声明，文件必须包含 marker 之外的真实符号。
fn declared_ddd_layers_have_real_responsibilities() {
    let mut offenders = Vec::new();

    for context in backend_module_contexts() {
        let module_path = format!("src/modules/{context}/mod.rs");
        let module_source = fs::read_to_string(&module_path).expect("read context module file");
        for layer in DDD_LAYERS {
            let path = format!("src/modules/{context}/{layer}.rs");
            let declaration = format!("pub mod {layer};");
            let restricted_declaration = format!("pub(crate) mod {layer};");
            let declared = module_source.lines().any(|line| {
                let line = code_line(line);
                line == declaration || line == restricted_declaration
            });
            let exists = Path::new(&path).exists();
            if declared && !exists {
                offenders.push(format!("{path} (declared but missing)"));
                continue;
            }
            if !declared && exists {
                offenders.push(format!("{path} (exists but is not declared)"));
                continue;
            }
            if !declared {
                continue;
            }
            let source = fs::read_to_string(&path).expect("read DDD layer file");
            if !has_real_layer_symbol(&source) {
                offenders.push(path);
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "declared DDD layers must contain real responsibilities; delete empty layers and their mod declarations: {offenders:?}"
    );
}

#[test]
/// 不得用 `*LayerMarker` 伪装分层完成；真实类型仍可实现 architecture traits。
fn layer_markers_are_forbidden() {
    let mut offenders = Vec::new();
    collect_layer_marker_offenders(Path::new("src"), &mut offenders);

    assert!(
        offenders.is_empty(),
        "layer markers are forbidden; implement architecture traits on real types or omit the optional layer: {offenders:?}"
    );
}

#[test]
/// 校验生产源码中没有将测试体内嵌在 `mod tests { ... }` 里。
fn production_sources_do_not_embed_test_bodies() {
    let mut offenders = Vec::new();
    collect_inline_test_offenders(Path::new("src"), &mut offenders);

    assert!(
        offenders.is_empty(),
        "test bodies must live in standalone files, found inline tests in: {offenders:?}"
    );
}

#[test]
/// 校验生产源码中的测试模块声明只允许引用 `tests/unit_src` 下的独立测试文件。
fn production_sources_must_reference_unit_test_files() {
    let mut offenders = Vec::new();
    collect_unit_test_references(Path::new("src"), &mut offenders);

    assert!(
        offenders.is_empty(),
        "test modules in production source must only load dedicated unit test files under tests/unit_src, found: {offenders:?}"
    );
}

#[test]
/// routes 仅做传输适配，禁止原始 SQL、事务所有权、直连 infrastructure 和 provider HTTP。
fn routes_obey_transport_boundary() {
    let mut offenders = Vec::new();
    collect_route_dependency_offenders(Path::new("src/modules"), &mut offenders);
    assert_dependency_rule("routes.", offenders);
}

#[test]
/// domain 仅保留纯规则，禁止传输、存储、provider SDK 及 presentation 反向依赖。
fn domain_layers_are_sdk_independent() {
    let mut offenders = Vec::new();
    collect_domain_dependency_offenders(Path::new("src/modules"), &mut offenders);
    assert_dependency_rule("domain.", offenders);
}

#[test]
/// repository 定义持久化契约，具体 SQL 必须归 infrastructure。
fn repository_layers_do_not_own_concrete_sql() {
    let mut offenders = Vec::new();
    collect_repository_dependency_offenders(Path::new("src/modules"), &mut offenders);
    assert_dependency_rule("repository.", offenders);
}

#[test]
/// service 不得反向依赖 application 或 routes。
fn service_layers_do_not_depend_on_orchestration_or_routes() {
    let mut offenders = Vec::new();
    collect_service_dependency_offenders(Path::new("src/modules"), &mut offenders);
    assert_dependency_rule("service.", offenders);
}

#[test]
/// 生产源码单文件不得超过 2,000 行；达到边界前应按真实职责拆成子模块并保留兼容 façade。
fn production_rust_files_stay_below_2000_lines() {
    let mut offenders = Vec::new();
    visit_rust_files(Path::new("src"), &mut |path, source| {
        let line_count = source.lines().count();
        if line_count > 2_000 {
            offenders.push(format!("{} -> {line_count} lines", path.display()));
        }
    });

    assert!(
        offenders.is_empty(),
        "production Rust files must not exceed 2,000 lines; split by real responsibilities and retain a compatibility façade when required: {offenders:?}"
    );
}

fn backend_module_contexts() -> Vec<String> {
    let mut contexts = fs::read_dir("src/modules")
        .expect("read src/modules directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            if file_type.is_dir() {
                Some(entry.file_name().to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    contexts.sort();
    contexts
}

fn has_real_layer_symbol(source: &str) -> bool {
    source.lines().any(|line| {
        let line = code_line(line);
        if line.is_empty() || line.contains("LayerMarker") {
            return false;
        }

        [
            "fn ", "struct ", "enum ", "trait ", "type ", "const ", "static ", "mod ",
        ]
        .iter()
        .any(|declaration| contains_identifier_declaration(line, declaration))
    })
}

fn contains_identifier_declaration(line: &str, declaration: &str) -> bool {
    line.starts_with(declaration)
        || line.starts_with(&format!("pub {declaration}"))
        || line.starts_with(&format!("pub(crate) {declaration}"))
        || line.starts_with(&format!("pub(super) {declaration}"))
        || line.starts_with(&format!("pub async {declaration}"))
        || line.starts_with(&format!("pub(crate) async {declaration}"))
        || line.starts_with(&format!("pub(super) async {declaration}"))
        || line.starts_with(&format!("async {declaration}"))
}

fn collect_layer_marker_offenders(dir: &Path, offenders: &mut Vec<String>) {
    visit_rust_files(dir, &mut |path, source| {
        for (line_number, line) in source.lines().enumerate() {
            let code = code_line(line);
            let marker_identifiers = code
                .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                .filter(|identifier| identifier.ends_with("LayerMarker"))
                .collect::<BTreeSet<_>>();
            if marker_identifiers.is_empty() {
                continue;
            }
            offenders.push(format!(
                "{}:{} -> {}",
                path.display(),
                line_number + 1,
                marker_identifiers
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    });
}

fn collect_inline_test_offenders(dir: &Path, offenders: &mut Vec<String>) {
    visit_rust_files(dir, &mut |path, source| {
        if source.contains("mod tests {") {
            offenders.push(path.display().to_string());
        }
    });
}

fn collect_unit_test_references(dir: &Path, offenders: &mut Vec<String>) {
    visit_rust_files(dir, &mut |path, source| {
        let lines = source.lines().collect::<Vec<_>>();
        for i in 0..lines.len() {
            if lines[i].trim() != "#[cfg(test)]" {
                continue;
            }
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if j >= lines.len() {
                offenders.push(path.display().to_string());
                continue;
            }

            let next = lines[j].trim();
            if next.starts_with("#[path =") {
                let Some(attr_start) = next.find('"') else {
                    offenders.push(format!("{} -> invalid #[path] attribute", path.display()));
                    continue;
                };
                let Some(attr_end) = next[attr_start + 1..].find('"') else {
                    offenders.push(format!("{} -> invalid #[path] attribute", path.display()));
                    continue;
                };
                let candidate = &next[attr_start + 1..attr_start + 1 + attr_end];
                let referenced_path = Path::new(candidate);
                let mut k = j + 1;
                while k < lines.len() && lines[k].trim().is_empty() {
                    k += 1;
                }
                let next_non_empty = lines.get(k).map(|line| line.trim()).unwrap_or("");
                if !next_non_empty.starts_with("mod tests")
                    || !candidate.contains("tests/unit_src/")
                {
                    offenders.push(format!(
                        "{} -> test module declaration not pointing to tests/unit_src",
                        path.display()
                    ));
                    continue;
                }
                let referenced_abs = path.parent().unwrap_or(Path::new("")).join(referenced_path);
                if !referenced_abs.exists() {
                    offenders.push(format!(
                        "{} -> missing referenced test file {}",
                        path.display(),
                        referenced_path.display()
                    ));
                }
                continue;
            }

            if next.starts_with("mod tests") {
                offenders.push(format!(
                    "{} -> inline test module declaration without #[path]",
                    path.display()
                ));
            }
        }
    });
}

fn collect_route_dependency_offenders(dir: &Path, offenders: &mut Vec<DependencyOffender>) {
    visit_named_rust_files(dir, "routes.rs", &mut |path, source| {
        for (line_number, line) in source.lines().enumerate() {
            let code = code_line(line);
            let checks = [
                ("routes.raw_sql", "sqlx", contains_identifier(code, "sqlx")),
                (
                    "routes.raw_sql",
                    "QueryBuilder",
                    contains_identifier(code, "QueryBuilder"),
                ),
                ("routes.transaction", ".begin(", code.contains(".begin(")),
                (
                    "routes.transaction",
                    "Transaction",
                    contains_identifier(code, "Transaction"),
                ),
                (
                    "routes.direct_infrastructure",
                    "infrastructure",
                    contains_identifier(code, "infrastructure"),
                ),
                (
                    "routes.provider_http",
                    "reqwest",
                    contains_identifier(code, "reqwest"),
                ),
            ];
            push_dependency_matches(path, line_number, code, &checks, offenders);
        }
    });
}

fn collect_domain_dependency_offenders(dir: &Path, offenders: &mut Vec<DependencyOffender>) {
    visit_named_rust_files(dir, "domain.rs", &mut |path, source| {
        for (line_number, line) in source.lines().enumerate() {
            let code = code_line(line);
            let mut checks = Vec::new();
            for dependency in ["axum", "sqlx", "redis", "mongodb", "reqwest"] {
                checks.push((
                    "domain.storage_sdk",
                    dependency,
                    contains_identifier(code, dependency),
                ));
            }
            checks.push((
                "domain.presentation",
                "presentation",
                contains_identifier(code, "presentation"),
            ));
            push_dependency_matches(path, line_number, code, &checks, offenders);
        }
    });
}

fn collect_repository_dependency_offenders(dir: &Path, offenders: &mut Vec<DependencyOffender>) {
    visit_named_rust_files(dir, "repository.rs", &mut |path, source| {
        for (line_number, line) in source.lines().enumerate() {
            let code = code_line(line);
            let checks = [
                (
                    "repository.concrete_sql",
                    "sqlx::query",
                    code.contains("sqlx::query"),
                ),
                (
                    "repository.concrete_sql",
                    "QueryBuilder",
                    contains_identifier(code, "QueryBuilder"),
                ),
            ];
            push_dependency_matches(path, line_number, code, &checks, offenders);
        }
    });
}

fn collect_service_dependency_offenders(dir: &Path, offenders: &mut Vec<DependencyOffender>) {
    visit_named_rust_files(dir, "service.rs", &mut |path, source| {
        for (line_number, line) in source.lines().enumerate() {
            let code = code_line(line);
            let checks = [
                (
                    "service.application",
                    "application",
                    contains_path_segment(code, "application"),
                ),
                (
                    "service.routes",
                    "routes",
                    contains_path_segment(code, "routes"),
                ),
            ];
            push_dependency_matches(path, line_number, code, &checks, offenders);
        }
    });
}

#[derive(Debug)]
struct DependencyOffender {
    path: String,
    line_number: usize,
    rule: &'static str,
    evidence: String,
}

fn push_dependency_matches(
    path: &Path,
    line_number: usize,
    code: &str,
    checks: &[(&'static str, &str, bool)],
    offenders: &mut Vec<DependencyOffender>,
) {
    for (rule, _, matched) in checks {
        if *matched {
            offenders.push(DependencyOffender {
                path: path.display().to_string(),
                line_number: line_number + 1,
                rule,
                evidence: code.to_owned(),
            });
        }
    }
}

fn assert_dependency_rule(rule_prefix: &str, offenders: Vec<DependencyOffender>) {
    let violations = offenders
        .into_iter()
        .filter(|offender| offender.rule.starts_with(rule_prefix))
        .map(|offender| {
            format!(
                "{}:{} [{}] {}",
                offender.path, offender.line_number, offender.rule, offender.evidence
            )
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "DDD dependency violations found: {violations:?}"
    );
}

fn contains_identifier(line: &str, expected: &str) -> bool {
    line.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|identifier| identifier == expected)
}

fn contains_path_segment(line: &str, expected: &str) -> bool {
    line.split("::").any(|segment| {
        segment
            .trim()
            .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
            == expected
    })
}

fn code_line(line: &str) -> &str {
    let line = line.trim();
    if line.starts_with("//") {
        return "";
    }
    line.split_once("//").map_or(line, |(code, _)| code).trim()
}

fn visit_named_rust_files(dir: &Path, file_name: &str, visitor: &mut impl FnMut(&Path, &str)) {
    visit_rust_files(dir, &mut |path, source| {
        if path.file_name().and_then(|name| name.to_str()) == Some(file_name) {
            visitor(path, source);
        }
    });
}

fn visit_rust_files(dir: &Path, visitor: &mut impl FnMut(&Path, &str)) {
    for entry in fs::read_dir(dir).expect("read Rust source directory") {
        let entry = entry.expect("read Rust source directory entry");
        let path = entry.path();
        if path.is_dir() {
            visit_rust_files(&path, visitor);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read Rust source file");
        visitor(&path, &source);
    }
}
