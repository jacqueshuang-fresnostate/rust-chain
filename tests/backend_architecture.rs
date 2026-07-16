use std::{fs, path::Path};

const DDD_LAYERS: &[&str] = &[
    "domain",
    "repository",
    "service",
    "application",
    "infrastructure",
    "presentation",
];

#[test]
/// 校验核心上下文必须保留完整 DDD 分层文件，不允许新增模块缺层。
fn backend_contexts_have_ddd_layer_files() {
    for context in backend_module_contexts() {
        for layer in DDD_LAYERS {
            let path = format!("src/modules/{context}/{layer}.rs");
            assert!(Path::new(&path).exists(), "missing DDD layer file: {path}");
        }
    }
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
/// 校验路由层对 service 层的依赖是“可控白名单”以免回退为业务逻辑越权依赖。
fn routes_should_only_reference_whitelisted_service_symbols() {
    let mut offenders = Vec::new();
    collect_route_service_import_offenders(Path::new("src/modules"), &mut offenders);

    assert!(
        offenders.is_empty(),
        "route files reference unsupported service identifiers: {offenders:?}"
    );
}

fn collect_inline_test_offenders(dir: &Path, offenders: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("read source directory") {
        let entry = entry.expect("read source directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_inline_test_offenders(&path, offenders);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read Rust source file");
        if source.contains("mod tests {") {
            offenders.push(path.display().to_string());
        }
    }
}

fn collect_unit_test_references(dir: &Path, offenders: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("read source directory") {
        let entry = entry.expect("read source directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_unit_test_references(&path, offenders);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read Rust source file");
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
                let Some(attr_start) = next.find('\"') else {
                    offenders.push(format!("{} -> invalid #[path] attribute", path.display()));
                    continue;
                };
                let Some(attr_end) = next[attr_start + 1..].find('\"') else {
                    offenders.push(format!("{} -> invalid #[path] attribute", path.display()));
                    continue;
                };
                let candidate = &next[attr_start + 1..attr_start + 1 + attr_end];
                let referenced_path = Path::new(&candidate);
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
    }
}

fn collect_route_service_import_offenders(dir: &Path, offenders: &mut Vec<String>) {
    const ALLOWED_SERVICE_SYMBOLS: &[&str] = &[
        "admin_id_from_subject",
        "load_market_feed_runtime",
        "MAX_UPLOAD_BODY_SIZE_BYTES",
        "mysql_pool",
        "public_ws_confirmation_text",
        "run_private_socket",
        "run_public_multi_socket",
        "run_public_socket",
        "route_limit",
        "user_id_from_subject",
    ];

    for entry in fs::read_dir(dir).expect("read source directory") {
        let entry = entry.expect("read source directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_route_service_import_offenders(&path, offenders);
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) != Some("routes.rs") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read Rust source file");
        let mut collecting_inline_brace = false;
        let mut brace_buffer = String::new();

        for line in source.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            if line.contains("service::{") {
                collecting_inline_brace = true;
            }
            if collecting_inline_brace {
                brace_buffer.push_str(line);
                brace_buffer.push('\n');
                if line.contains('}') {
                    collecting_inline_brace = false;
                }
            }

            if !line.contains("service::") || line.contains("service::{") {
                continue;
            }
            if let Some(symbol_start) = line.find("service::") {
                let symbol = line[symbol_start + "service::".len()..]
                    .chars()
                    .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                    .collect::<String>();
                if !symbol.is_empty() && !ALLOWED_SERVICE_SYMBOLS.contains(&symbol.as_str()) {
                    offenders.push(format!("{} -> {}", path.display(), symbol));
                }
            }
        }

        for block in brace_buffer.split("service::{") {
            let Some(partial) = block.split_once('}') else {
                continue;
            };
            for entry in partial.0.split(',') {
                let symbol = entry
                    .split("as")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches(|ch: char| ch == '\r' || ch == '\n' || ch == ' ' || ch == '{');
                let symbol = symbol
                    .trim()
                    .trim_matches(|ch: char| ch == '(' || ch == ')' || ch == ';');
                if symbol.is_empty() {
                    continue;
                }
                if !ALLOWED_SERVICE_SYMBOLS.contains(&symbol) {
                    offenders.push(format!("{} -> {}", path.display(), symbol));
                }
            }
        }
    }
}
