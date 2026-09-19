use std::{fs, path::Path};
use syn::{spanned::Spanned, visit::Visit};

fn decimal_depth(ty: &syn::Type) -> Option<usize> {
    let syn::Type::Path(path) = ty else { return None };
    let last = path.path.segments.last()?;
    if last.ident == "BigDecimal" {
        return Some(0);
    }
    if last.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &last.arguments else { return None };
    let syn::GenericArgument::Type(inner) = arguments.args.first()? else { return None };
    decimal_depth(inner).map(|depth| depth + 1)
}

struct Scanner<'a> { path: &'a str }
impl<'ast> Visit<'ast> for Scanner<'_> {
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        let deserialize = item.attrs.iter().any(|attr| {
            attr.path().is_ident("derive") && attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            ).is_ok_and(|paths| paths.iter().any(|path| path.is_ident("Deserialize")))
        });
        if deserialize {
            for field in &item.fields {
                let Some(depth) = decimal_depth(&field.ty) else { continue };
                let Some(name) = &field.ident else { continue };
                let attrs = field.attrs.iter().filter(|attr| attr.path().is_ident("serde"))
                    .map(|attr| format!("{}-{}", attr.span().start().line, attr.span().end().line))
                    .collect::<Vec<_>>().join(",");
                println!("{}|{}|{}|{}|{}|{}", self.path, item.ident, name, depth, name.span().start().line, attrs);
            }
        }
        syn::visit::visit_item_struct(self, item);
    }
}

fn walk(path: &Path) {
    if path.is_dir() {
        let mut children = fs::read_dir(path).unwrap().map(|entry| entry.unwrap().path()).collect::<Vec<_>>();
        children.sort();
        for child in children { walk(&child); }
    } else if path.extension().is_some_and(|extension| extension == "rs") {
        let source = fs::read_to_string(path).unwrap();
        let ast = syn::parse_file(&source).unwrap();
        Scanner { path: path.to_str().unwrap() }.visit_file(&ast);
    }
}

fn main() { walk(Path::new("src/modules")); }
