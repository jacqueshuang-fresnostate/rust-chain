use proc_macro2::Span;
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use syn::{
    Attribute, Expr, ImplItemFn, ItemFn, Lit, Meta, TraitItemFn, Visibility,
    spanned::Spanned,
    visit::{self, Visit},
};
use walkdir::WalkDir;

const HIGH_RISK_ENTRY_POINTS: &[(&str, &str)] = &[
    (
        "src/modules/prediction/infrastructure.rs",
        "create_order_in_tx",
    ),
    (
        "src/modules/prediction/infrastructure.rs",
        "settle_market_in_tx",
    ),
    (
        "src/modules/prediction/infrastructure.rs",
        "apply_wallet_prediction_open",
    ),
    (
        "src/modules/prediction/infrastructure.rs",
        "apply_wallet_prediction_settlement",
    ),
    (
        "src/modules/prediction/infrastructure.rs",
        "apply_wallet_prediction_refund",
    ),
    (
        "src/modules/wallet/infrastructure/withdrawals.rs",
        "reserve_withdrawal_request",
    ),
    (
        "src/modules/wallet/infrastructure/withdrawals.rs",
        "release_withdrawal_in_tx",
    ),
    (
        "src/modules/wallet/infrastructure/withdrawals.rs",
        "confirm_withdrawal_in_tx",
    ),
    (
        "src/modules/wallet/infrastructure/deposits.rs",
        "observe_deposit_event",
    ),
    (
        "src/modules/wallet/infrastructure/deposits.rs",
        "reverse_deposit_event",
    ),
    (
        "src/modules/spot/application/settlement.rs",
        "settle_spot_fill",
    ),
    (
        "src/modules/spot/infrastructure/wallet_accounts.rs",
        "apply_spot_wallet_freeze",
    ),
    (
        "src/modules/spot/infrastructure/wallet_accounts.rs",
        "apply_spot_wallet_settlement_leg",
    ),
    (
        "src/modules/margin/application/open_position.rs",
        "open_margin_position",
    ),
    (
        "src/modules/margin/infrastructure/settlement.rs",
        "debit_margin_position_open_collateral",
    ),
    (
        "src/modules/margin/infrastructure/settlement.rs",
        "credit_margin_position_amount",
    ),
    (
        "src/workers/margin_liquidation.rs",
        "liquidate_cross_account",
    ),
    ("src/modules/seconds_contract/application.rs", "open_order"),
    (
        "src/modules/seconds_contract/application.rs",
        "settle_order",
    ),
    (
        "src/modules/quick_recharge/application.rs",
        "handle_gmpay_notify",
    ),
    ("src/workers/wallet_chain.rs", "run_once_with_gateway"),
];

#[derive(Debug)]
struct FunctionAudit {
    path: String,
    name: String,
    name_line: usize,
    end_line: usize,
    is_visible: bool,
    is_trait_method: bool,
    has_chinese_doc: bool,
    chinese_doc_chars: usize,
    documentation: String,
}

impl FunctionAudit {
    fn line_count(&self) -> usize {
        self.end_line.saturating_sub(self.name_line) + 1
    }

    fn is_public_responsibility(&self) -> bool {
        self.is_visible || self.is_trait_method
    }
}

#[derive(Default)]
struct FunctionCollector {
    path: String,
    functions: Vec<FunctionAudit>,
}

impl FunctionCollector {
    fn push(
        &mut self,
        name: &syn::Ident,
        visibility: &Visibility,
        attrs: &[Attribute],
        span: Span,
        is_trait_method: bool,
    ) {
        let doc_lines = documentation_lines(attrs);
        self.functions.push(FunctionAudit {
            path: self.path.clone(),
            name: name.to_string(),
            name_line: name.span().start().line,
            end_line: span.end().line,
            is_visible: !matches!(visibility, Visibility::Inherited),
            is_trait_method,
            has_chinese_doc: doc_lines.iter().any(|line| contains_chinese(line)),
            chinese_doc_chars: doc_lines
                .iter()
                .flat_map(|line| line.chars())
                .filter(|character| matches!(*character as u32, 0x3400..=0x9fff))
                .count(),
            documentation: doc_lines.join("\n"),
        });
    }
}

impl<'ast> Visit<'ast> for FunctionCollector {
    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        self.push(&item.sig.ident, &item.vis, &item.attrs, item.span(), false);
        visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        self.push(&item.sig.ident, &item.vis, &item.attrs, item.span(), false);
        visit::visit_impl_item_fn(self, item);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast TraitItemFn) {
        self.push(
            &item.sig.ident,
            &Visibility::Inherited,
            &item.attrs,
            item.span(),
            true,
        );
        visit::visit_trait_item_fn(self, item);
    }
}

#[test]
/// 风险方法、worker 和跨上下文基础设施入口必须携带自身中文职责文档。
fn backend_methods_have_executable_chinese_documentation_gate() {
    let functions = collect_source_functions(Path::new("src"));
    let visible_responsibilities = functions
        .iter()
        .filter(|function| {
            is_bounded_context_responsibility_layer(&function.path)
                && function.is_public_responsibility()
        })
        .collect::<Vec<_>>();
    let risk_name = Regex::new(concat!(
        "(?i)(wallet|balance|ledger|withdraw|deposit|settle|settlement|liquidat|",
        "margin|collateral|interest|loan|repay|order|trade|fill|price|fee|commission|",
        "recharge|convert|unlock|fund|auth|login|password|token|session|permission|scope|",
        "two_factor|totp|security|risk|idemp|transaction|in_tx|referral|kyc|audit|",
        "reserve|freeze|unfreeze|credit|debit)"
    ))
    .expect("risk function name regex must compile");
    let high_risk = HIGH_RISK_ENTRY_POINTS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut seen_high_risk = BTreeMap::<(&str, &str), Vec<&FunctionAudit>>::new();
    let mut violations = BTreeMap::<(String, usize, String), BTreeSet<&str>>::new();

    collect_repeated_documentation_violations(&visible_responsibilities, &mut violations);

    for function in &functions {
        if is_bounded_context_responsibility_layer(&function.path)
            && function.is_public_responsibility()
        {
            if !function.has_chinese_doc {
                add_violation(
                    &mut violations,
                    function,
                    "bounded context 的公开职责方法或 trait 方法缺少自身中文 doc",
                );
            } else if function.line_count() >= 15 && function.chinese_doc_chars < 24 {
                add_violation(
                    &mut violations,
                    function,
                    "长度至少 15 行的公开职责方法需要至少 24 个中文字符的详细合同说明",
                );
            }
        }

        if function.is_public_responsibility()
            && function.line_count() >= 50
            && risk_name.is_match(&function.name)
            && !function.has_chinese_doc
        {
            add_violation(
                &mut violations,
                function,
                "名称命中审计风险词且长度至少 50 行的非 private/trait 方法缺少自身中文 doc",
            );
        }

        if is_worker_or_cross_context_infra(&function.path)
            && function.is_public_responsibility()
            && !function.has_chinese_doc
        {
            add_violation(
                &mut violations,
                function,
                "src/workers 或 src/infra 的公开职责方法缺少自身中文 doc",
            );
        }

        let key = (function.path.as_str(), function.name.as_str());
        if high_risk.contains(&key) {
            seen_high_risk.entry(key).or_default().push(function);
            if !function.has_chinese_doc {
                add_violation(
                    &mut violations,
                    function,
                    "审计 P0 高风险入口缺少自身中文 doc",
                );
            }
        }
    }

    for &(path, name) in HIGH_RISK_ENTRY_POINTS {
        match seen_high_risk.get(&(path, name)) {
            None => {
                violations
                    .entry((path.to_owned(), 0, name.to_owned()))
                    .or_default()
                    .insert("审计 P0 高风险入口不存在，需同步更新 path+name 门禁");
            }
            Some(matches) if matches.len() != 1 => {
                for function in matches {
                    add_violation(
                        &mut violations,
                        function,
                        "审计 P0 path+name 匹配到多个方法，门禁必须保持唯一",
                    );
                }
            }
            Some(_) => {}
        }
    }

    assert!(
        violations.is_empty(),
        "backend Chinese documentation gate failed:\n{}",
        format_violations(&violations)
    );
}

/// 重复的整段 doc 会让“每个职责都有注释”退化为批量粘贴；同一文件内的非平凡入口必须拥有可区分合同。
/// 短 getter 可共享精确说明，跨文件的共同并发/事务约束也不在此门禁内误报。
fn collect_repeated_documentation_violations(
    functions: &[&FunctionAudit],
    violations: &mut BTreeMap<(String, usize, String), BTreeSet<&'static str>>,
) {
    let mut repeated = BTreeMap::<(&str, &str), Vec<&FunctionAudit>>::new();
    for function in functions {
        if function.line_count() < 6 || function.documentation.trim().is_empty() {
            continue;
        }
        repeated
            .entry((&function.path, function.documentation.trim()))
            .or_default()
            .push(function);
    }

    for matches in repeated.values().filter(|matches| matches.len() >= 4) {
        for function in matches {
            add_violation(
                violations,
                function,
                "同一文件至少 4 个非平凡公开职责复用完全相同 doc，需按真实输入、失败或副作用区分合同",
            );
        }
    }
}

fn collect_source_functions(root: &Path) -> Vec<FunctionAudit> {
    let mut paths = WalkDir::new(root)
        .into_iter()
        .map(|entry| entry.unwrap_or_else(|error| panic!("walk src failed: {error}")))
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("rs"))
        .filter(|path| !path.components().any(|part| part.as_os_str() == "target"))
        .collect::<Vec<PathBuf>>();
    paths.sort();

    let mut functions = Vec::new();
    for path in paths {
        let relative = normalized_path(&path);
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"));
        let file = syn::parse_file(&source)
            .unwrap_or_else(|error| panic!("failed to parse {relative}: {error}"));
        let mut collector = FunctionCollector {
            path: relative,
            ..FunctionCollector::default()
        };
        collector.visit_file(&file);
        functions.extend(collector.functions);
    }

    functions.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.name_line.cmp(&right.name_line))
            .then(left.name.cmp(&right.name))
    });
    functions
}

fn documentation_lines(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }
            match &attr.meta {
                Meta::NameValue(value) => match &value.value {
                    Expr::Lit(expr) => match &expr.lit {
                        Lit::Str(doc) => Some(doc.value()),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            }
        })
        .collect()
}

fn contains_chinese(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character as u32, 0x3400..=0x9fff))
}

fn is_worker_or_cross_context_infra(path: &str) -> bool {
    path.starts_with("src/workers/") || path.starts_with("src/infra/")
}

fn is_bounded_context_responsibility_layer(path: &str) -> bool {
    if !path.starts_with("src/modules/") {
        return false;
    }

    [
        "/domain.rs",
        "/domain/",
        "/application.rs",
        "/application/",
        "/service.rs",
        "/service/",
        "/infrastructure.rs",
        "/infrastructure/",
        "/repository.rs",
        "/repository/",
    ]
    .iter()
    .any(|segment| path.contains(segment))
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn add_violation<'a>(
    violations: &mut BTreeMap<(String, usize, String), BTreeSet<&'a str>>,
    function: &FunctionAudit,
    reason: &'a str,
) {
    violations
        .entry((
            function.path.clone(),
            function.name_line,
            function.name.clone(),
        ))
        .or_default()
        .insert(reason);
}

fn format_violations(violations: &BTreeMap<(String, usize, String), BTreeSet<&str>>) -> String {
    violations
        .iter()
        .map(|((path, line, name), reasons)| {
            format!(
                "- {path}:{line} `{name}`: {}",
                reasons.iter().copied().collect::<Vec<_>>().join("; ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
