//! Format rustdoc types, generics, and function signatures back into readable
//! Rust source fragments. These are emitted inside fenced code blocks, so they
//! are plain text (no links).

use rustdoc_types::{
    Function, FunctionSignature, GenericArg, GenericArgs, GenericBound, GenericParamDef,
    GenericParamDefKind, Generics, Path, Term, TraitBoundModifier, Type, WherePredicate,
};

/// Format a full function signature (without a visibility keyword), e.g.
/// `fn map<U>(self, f: F) -> Option<U>`.
pub fn function_signature(name: &str, func: &Function) -> String {
    let mut s = String::new();
    if func.header.is_const {
        s.push_str("const ");
    }
    if func.header.is_async {
        s.push_str("async ");
    }
    if func.header.is_unsafe {
        s.push_str("unsafe ");
    }
    s.push_str("fn ");
    s.push_str(name);
    s.push_str(&generics_decl(&func.generics));
    s.push_str(&fn_params_and_ret(&func.sig));
    s.push_str(&where_clause(&func.generics));
    s
}

/// Render a [`Type`] as Rust source.
pub fn type_str(ty: &Type) -> String {
    match ty {
        Type::ResolvedPath(path) => path_str(path),
        Type::Generic(name) => name.clone(),
        Type::Primitive(name) => name.clone(),
        Type::Infer => "_".to_string(),
        Type::Tuple(elems) => {
            let inner: Vec<String> = elems.iter().map(type_str).collect();
            format!("({})", inner.join(", "))
        }
        Type::Slice(inner) => format!("[{}]", type_str(inner)),
        Type::Array { type_, len } => format!("[{}; {len}]", type_str(type_)),
        Type::ImplTrait(bounds) => format!("impl {}", bounds_str(bounds)),
        Type::DynTrait(dyn_trait) => {
            let traits: Vec<String> = dyn_trait
                .traits
                .iter()
                .map(|pt| path_str(&pt.trait_))
                .collect();
            let mut s = format!("dyn {}", traits.join(" + "));
            if let Some(lt) = &dyn_trait.lifetime {
                s.push_str(&format!(" + {lt}"));
            }
            s
        }
        Type::RawPointer { is_mutable, type_ } => {
            let kw = if *is_mutable { "*mut" } else { "*const" };
            format!("{kw} {}", type_str(type_))
        }
        Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_,
        } => {
            let mut s = "&".to_string();
            if let Some(lt) = lifetime {
                s.push_str(&format!("{lt} "));
            }
            if *is_mutable {
                s.push_str("mut ");
            }
            s.push_str(&type_str(type_));
            s
        }
        Type::QualifiedPath {
            name,
            self_type,
            trait_,
            ..
        } => match trait_ {
            // `<T as Trait>::Name`; but rustdoc leaves the trait path empty for
            // plain `Self::Name` / `T::Name`, which we render without the cast.
            Some(tr) if !tr.path.is_empty() => {
                format!("<{} as {}>::{name}", type_str(self_type), path_str(tr))
            }
            _ => format!("{}::{name}", type_str(self_type)),
        },
        Type::FunctionPointer(fp) => {
            format!("fn{}", fn_params_and_ret(&fp.sig))
        }
        Type::Pat { type_, .. } => type_str(type_),
    }
}

/// Render a resolved path with its generic arguments, e.g. `Vec<T>`.
fn path_str(path: &Path) -> String {
    format!("{}{}", path.path, generic_args(path.args.as_deref()))
}

fn generic_args(args: Option<&GenericArgs>) -> String {
    match args {
        None => String::new(),
        Some(GenericArgs::AngleBracketed { args, constraints }) => {
            let mut parts: Vec<String> = args.iter().map(generic_arg).collect();
            for c in constraints {
                // e.g. `Item = u32`
                if let Term::Type(ty) = &c.binding_term() {
                    parts.push(format!("{} = {}", c.name, type_str(ty)));
                }
            }
            if parts.is_empty() {
                String::new()
            } else {
                format!("<{}>", parts.join(", "))
            }
        }
        Some(GenericArgs::Parenthesized { inputs, output }) => {
            let ins: Vec<String> = inputs.iter().map(type_str).collect();
            let mut s = format!("({})", ins.join(", "));
            if let Some(out) = output {
                s.push_str(&format!(" -> {}", type_str(out)));
            }
            s
        }
        Some(GenericArgs::ReturnTypeNotation) => "(..)".to_string(),
    }
}

fn generic_arg(arg: &GenericArg) -> String {
    match arg {
        GenericArg::Lifetime(lt) => lt.clone(),
        GenericArg::Type(ty) => type_str(ty),
        GenericArg::Const(c) => c.expr.clone(),
        GenericArg::Infer => "_".to_string(),
    }
}

/// Render a list of bounds joined with ` + `.
pub fn bounds_str(bounds: &[GenericBound]) -> String {
    bounds
        .iter()
        .map(bound_str)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" + ")
}

fn bound_str(bound: &GenericBound) -> String {
    match bound {
        GenericBound::TraitBound {
            trait_, modifier, ..
        } => {
            let prefix = match modifier {
                TraitBoundModifier::None => "",
                TraitBoundModifier::Maybe => "?",
                TraitBoundModifier::MaybeConst => "~const ",
            };
            format!("{prefix}{}", path_str(trait_))
        }
        GenericBound::Outlives(lt) => lt.clone(),
        GenericBound::Use(_) => String::new(),
    }
}

/// The `<...>` generic parameter declaration for a definition, or empty.
/// Compiler-synthesized parameters (from `impl Trait` in argument position) are
/// omitted.
pub fn generics_decl(generics: &Generics) -> String {
    let params: Vec<String> = generics
        .params
        .iter()
        .filter(|p| !is_synthetic(p))
        .map(param_str)
        .collect();
    if params.is_empty() {
        String::new()
    } else {
        format!("<{}>", params.join(", "))
    }
}

fn is_synthetic(param: &GenericParamDef) -> bool {
    matches!(
        param.kind,
        GenericParamDefKind::Type {
            is_synthetic: true,
            ..
        }
    )
}

fn param_str(param: &GenericParamDef) -> String {
    match &param.kind {
        GenericParamDefKind::Lifetime { outlives } => {
            if outlives.is_empty() {
                param.name.clone()
            } else {
                format!("{}: {}", param.name, outlives.join(" + "))
            }
        }
        GenericParamDefKind::Type {
            bounds, default, ..
        } => {
            let mut s = param.name.clone();
            if !bounds.is_empty() {
                s.push_str(&format!(": {}", bounds_str(bounds)));
            }
            if let Some(def) = default {
                s.push_str(&format!(" = {}", type_str(def)));
            }
            s
        }
        GenericParamDefKind::Const { type_, default } => {
            let mut s = format!("const {}: {}", param.name, type_str(type_));
            if let Some(def) = default {
                s.push_str(&format!(" = {def}"));
            }
            s
        }
    }
}

/// The ` where ...` clause for a definition, or empty.
pub fn where_clause(generics: &Generics) -> String {
    let preds: Vec<String> = generics
        .where_predicates
        .iter()
        .map(predicate_str)
        .filter(|s| !s.is_empty())
        .collect();
    if preds.is_empty() {
        String::new()
    } else {
        format!("\nwhere\n    {}", preds.join(",\n    "))
    }
}

fn predicate_str(pred: &WherePredicate) -> String {
    match pred {
        WherePredicate::BoundPredicate { type_, bounds, .. } => {
            format!("{}: {}", type_str(type_), bounds_str(bounds))
        }
        WherePredicate::LifetimePredicate { lifetime, outlives } => {
            if outlives.is_empty() {
                lifetime.clone()
            } else {
                format!("{lifetime}: {}", outlives.join(" + "))
            }
        }
        WherePredicate::EqPredicate { lhs, rhs } => {
            let rhs = match rhs {
                Term::Type(ty) => type_str(ty),
                Term::Constant(c) => c.expr.clone(),
            };
            format!("{} = {rhs}", type_str(lhs))
        }
    }
}

/// The `(args) -> Ret` portion of a function signature.
pub fn fn_params_and_ret(sig: &FunctionSignature) -> String {
    let mut params: Vec<String> = sig
        .inputs
        .iter()
        .map(|(name, ty)| format_param(name, ty))
        .collect();
    if sig.is_c_variadic {
        params.push("...".to_string());
    }
    let mut s = format!("({})", params.join(", "));
    if let Some(output) = &sig.output {
        s.push_str(&format!(" -> {}", type_str(output)));
    }
    s
}

/// Format a single parameter, rendering the receiver as `self` / `&self` / `&mut self`.
fn format_param(name: &str, ty: &Type) -> String {
    if name == "self" {
        return match ty {
            Type::Generic(g) if g == "Self" => "self".to_string(),
            Type::BorrowedRef {
                lifetime,
                is_mutable,
                type_,
            } if matches!(type_.as_ref(), Type::Generic(g) if g == "Self") => {
                let lt = lifetime
                    .as_ref()
                    .map(|l| format!("{l} "))
                    .unwrap_or_default();
                let m = if *is_mutable { "mut " } else { "" };
                format!("&{lt}{m}self")
            }
            _ => format!("self: {}", type_str(ty)),
        };
    }
    format!("{name}: {}", type_str(ty))
}

// `AssocItemConstraint` exposes its term through the `binding` field; provide a
// small helper so the formatter above reads cleanly.
trait BindingTerm {
    fn binding_term(&self) -> Term;
}

impl BindingTerm for rustdoc_types::AssocItemConstraint {
    fn binding_term(&self) -> Term {
        match &self.binding {
            rustdoc_types::AssocItemConstraintKind::Equality(term) => term.clone(),
            rustdoc_types::AssocItemConstraintKind::Constraint(_) => {
                // A bound like `Item: Copy`; rendered elsewhere. Use a dummy.
                Term::Type(Type::Infer)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustdoc_types::{Id, Path};

    fn resolved(name: &str) -> Type {
        Type::ResolvedPath(Path {
            path: name.to_string(),
            id: Id(0),
            args: None,
        })
    }

    #[test]
    fn primitives_and_refs() {
        assert_eq!(type_str(&Type::Primitive("u32".into())), "u32");
        assert_eq!(
            type_str(&Type::BorrowedRef {
                lifetime: Some("'a".into()),
                is_mutable: true,
                type_: Box::new(Type::Primitive("str".into())),
            }),
            "&'a mut str"
        );
    }

    #[test]
    fn tuples_slices_arrays() {
        assert_eq!(
            type_str(&Type::Tuple(vec![
                Type::Primitive("u8".into()),
                resolved("String")
            ])),
            "(u8, String)"
        );
        assert_eq!(
            type_str(&Type::Slice(Box::new(Type::Primitive("u8".into())))),
            "[u8]"
        );
        assert_eq!(
            type_str(&Type::Array {
                type_: Box::new(Type::Primitive("u8".into())),
                len: "4".into(),
            }),
            "[u8; 4]"
        );
    }

    #[test]
    fn generic_path_args() {
        let ty = Type::ResolvedPath(Path {
            path: "Vec".into(),
            id: Id(0),
            args: Some(Box::new(GenericArgs::AngleBracketed {
                args: vec![GenericArg::Type(resolved("String"))],
                constraints: vec![],
            })),
        });
        assert_eq!(type_str(&ty), "Vec<String>");
    }
}
