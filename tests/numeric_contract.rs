use std::{fs, path::Path};
use syn::{spanned::Spanned, visit::Visit};

fn decimal_shape(ty: &syn::Type) -> Option<Vec<&'static str>> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    let last = path.path.segments.last()?;
    if last.ident == "BigDecimal" {
        return Some(Vec::new());
    }
    let wrapper = match last.ident.to_string().as_str() {
        "Option" => "Option",
        "Vec" => "Vec",
        _ => return None,
    };
    let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else {
        return None;
    };
    let syn::GenericArgument::Type(inner) = arguments.args.first()? else {
        return None;
    };
    decimal_shape(inner).map(|mut shape| {
        shape.insert(0, wrapper);
        shape
    })
}

struct Guard<'a> {
    path: &'a Path,
    source: &'a str,
    fields: usize,
    missing: Vec<String>,
}

impl<'ast> Visit<'ast> for Guard<'_> {
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        let name = item.ident.to_string();
        let nested_request = matches!(
            name.as_str(),
            "WithdrawFeeTier"
                | "WithdrawalAllowance"
                | "WithdrawalReviewTier"
                | "DefaultMarketParameters"
                | "DefaultMarketFollowParameters"
        );
        let transport = self.path.to_string_lossy().contains("presentation")
            && !name.ends_with("Response")
            && !name.contains("Cache");
        let is_input = transport || name.ends_with("Request") || nested_request;
        let deserialize = item.attrs.iter().any(|attr| {
            attr.path().is_ident("derive")
                && attr
                    .parse_args_with(
                        syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                    )
                    .is_ok_and(|paths| paths.iter().any(|path| path.is_ident("Deserialize")))
        });
        if is_input && deserialize {
            for field in &item.fields {
                let Some(shape) = decimal_shape(&field.ty) else {
                    continue;
                };
                self.fields += 1;
                let helper = match shape.as_slice() {
                    [] => "crate::numeric::deserialize_decimal",
                    ["Option"] => "crate::numeric::deserialize_optional_decimal",
                    ["Option", "Option"] => "crate::numeric::deserialize_patch_decimal",
                    ["Option", "Vec"] => "crate::numeric::deserialize_optional_decimals",
                    _ => panic!("unsupported financial request wrapper: {name} {shape:?}"),
                };
                let guarded = field.attrs.iter().any(|attr| {
                    if !attr.path().is_ident("serde") {
                        return false;
                    }
                    let span = attr.span();
                    self.source
                        .lines()
                        .skip(span.start().line - 1)
                        .take(span.end().line - span.start().line + 1)
                        .collect::<Vec<_>>()
                        .join("\n")
                        .contains(helper)
                });
                if !guarded {
                    self.missing.push(format!(
                        "{}:{} {}.{}",
                        self.path.display(),
                        field.span().start().line,
                        name,
                        field.ident.as_ref().unwrap()
                    ));
                }
            }
        }
        syn::visit::visit_item_struct(self, item);
    }
}

#[test]
fn financial_request_decimal_fields_use_bounded_lossless_deserialization() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/modules");
    let mut checked = 0;
    let mut missing = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.unwrap();
        if !entry.file_type().is_file()
            || entry
                .path()
                .extension()
                .is_none_or(|extension| extension != "rs")
        {
            continue;
        }
        let source = fs::read_to_string(entry.path()).unwrap();
        let ast = syn::parse_file(&source).unwrap();
        let mut guard = Guard {
            path: entry.path(),
            source: &source,
            fields: 0,
            missing: Vec::new(),
        };
        guard.visit_file(&ast);
        checked += guard.fields;
        missing.extend(guard.missing);
    }
    assert!(checked > 0, "financial DTO inventory must not be empty");
    assert!(
        missing.is_empty(),
        "unguarded financial inputs:\n{}",
        missing.join("\n")
    );
    println!("checked {checked} financial request/config fields");
}
