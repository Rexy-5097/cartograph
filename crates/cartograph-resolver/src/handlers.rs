//! Where a URL-conf entry's handler is declared (T-002).
//!
//! A decorator route names its handler by decorating it, so the handler is in
//! the file that declares the route. A Django URL conf does not: `urls.py`
//! says `path("orders/", views.list_orders)` and the function lives in
//! `views.py`. Naming the graph node after the *reference* put it in
//! `urls.py`, where nothing else points, while the handler's own model
//! accesses hang off the function in `views.py`. The chain from a request to
//! a table then stopped one hop short, at a node that is not a declaration.
//!
//! This module resolves the reference to the declaration, through the URL
//! conf's own imports, and nothing more:
//!
//! | Written in the URL conf | Resolved to |
//! |---|---|
//! | `list_orders`, after `from .views import list_orders` | the function |
//! | `views.list_orders`, after `from . import views` | the function |
//! | `v.list_orders`, after `from shop import views as v` or `import shop.views as v` | the function |
//! | `shop.views.list_orders`, after `import shop.views` | the function |
//! | `OrderView.as_view`, `views.OrderView.as_view` | the class |
//!
//! # What it refuses
//!
//! Anything else resolves to nothing, and the caller keeps the node it has
//! always made, named by the reference as written. In particular:
//!
//! - **A wrapper call** such as `login_required(views.orders)`. What the
//!   wrapper returns is not known here, so the wrapped view is not assumed to
//!   be the handler.
//! - **An ambiguous or unresolved import.** Never settled by picking one.
//! - **A name that is not a function or a class**, such as a module-scope
//!   `orders = OrderView.as_view()`.
//! - **`X.as_view` where `X` is not a class**, and a bare class used without
//!   `as_view`.
//!
//! # What it does not connect
//!
//! A class-based view resolves to its **class**. The methods Django
//! dispatches to (`get`, `post`, …) are separate nodes, and two classes in
//! one file that both define `get` share one node (ADR-0011), so joining a
//! class to "its" `get` would also join it to the other class's model
//! accesses. That edge is therefore not drawn.

use std::collections::HashMap;

use cartograph_core::NodeKind;
use cartograph_parser::model::{FileAnalysis, RouteDeclarationStyle, SourceLanguage, SymbolKind};

use crate::imports::{ModuleIndex, Resolution};

/// The declaration a URL-conf entry's handler reference names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerDeclaration {
    /// [`NodeKind::Function`] for a function view, [`NodeKind::Class`] for a
    /// class-based one.
    pub kind: NodeKind,
    /// Repository-relative file holding the declaration.
    pub file: String,
    /// The name the declaration has in that file.
    pub name: String,
    /// Line the declaration starts on.
    pub line: u32,
    /// How the reference was followed, for the edge's evidence.
    pub route: String,
}

/// Every URL-conf handler reference in a project that resolves to exactly one
/// declaration.
#[derive(Debug, Default)]
pub struct HandlerIndex {
    /// Keyed by the URL conf's file and the reference as written there.
    declarations: HashMap<(String, String), HandlerDeclaration>,
}

impl HandlerIndex {
    /// Resolves the handler of every URL-conf entry in `files`.
    #[must_use]
    pub fn build(files: &[FileAnalysis], modules: &ModuleIndex<'_>) -> Self {
        let mut declarations = HashMap::new();
        for file in files {
            if file.language != SourceLanguage::Python {
                continue;
            }
            for route in &file.routes {
                if route.style != RouteDeclarationStyle::UrlConfEntry {
                    continue;
                }
                let Some(reference) = &route.handler else {
                    continue;
                };
                let key = (file.path.clone(), reference.clone());
                if declarations.contains_key(&key) {
                    continue;
                }
                if let Some(declaration) = resolve(modules, file, reference) {
                    declarations.insert(key, declaration);
                }
            }
        }
        Self { declarations }
    }

    /// The declaration behind `reference` as written in `url_conf`, if it
    /// resolved.
    #[must_use]
    pub fn declaration(&self, url_conf: &str, reference: &str) -> Option<&HandlerDeclaration> {
        self.declarations
            .get(&(url_conf.to_owned(), reference.to_owned()))
    }

    /// How many references resolved.
    #[must_use]
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    /// Whether no reference resolved.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }
}

/// Resolves one handler reference written in `from`.
fn resolve(
    modules: &ModuleIndex<'_>,
    from: &FileAnalysis,
    reference: &str,
) -> Option<HandlerDeclaration> {
    // `OrderView.as_view` is the parser's record of `OrderView.as_view(...)`:
    // the view is the class, and `as_view` is how Django is handed it.
    let (target, class_based) = match reference.strip_suffix(".as_view") {
        Some(class) => (class, true),
        None => (reference, false),
    };
    let (file, name, line, route) = resolve_reference(modules, from, target)?;

    // The kind is read from the declaration, never assumed from the syntax.
    let declared = modules.file(&file)?.symbols.iter().find(|symbol| {
        symbol.name == name
            && symbol.enclosing.is_none()
            && matches!(symbol.kind, SymbolKind::Function | SymbolKind::Class)
    })?;
    let kind = match (declared.kind, class_based) {
        (SymbolKind::Function, false) => NodeKind::Function,
        (SymbolKind::Class, true) => NodeKind::Class,
        // `function.as_view` and a bare class are not forms Django accepts.
        _ => return None,
    };

    Some(HandlerDeclaration {
        kind,
        file,
        name,
        line,
        route,
    })
}

/// Follows a dotted Python reference written in `from` to the one
/// declaration it names: `(file, name, line, how it was reached)`.
///
/// `name` is resolved through `from`'s own bindings; `module.name` through
/// the import that binds `module`, then inside that module, following its
/// re-exports. Shared with ORM model discovery, which asks the same question
/// of a class's base (T-007). Anything that does not lead to exactly one
/// declaration is `None`.
pub(crate) fn resolve_reference(
    modules: &ModuleIndex<'_>,
    from: &FileAnalysis,
    target: &str,
) -> Option<(String, String, u32, String)> {
    if !is_dotted_name(target) {
        return None;
    }

    Some(match target.rsplit_once('.') {
        // A bare name: bound in the URL conf itself, or imported into it.
        None => match modules.resolve_python(from, target) {
            Resolution::Local { line } => (
                from.path.clone(),
                target.to_owned(),
                line,
                "declared in the URL conf".to_owned(),
            ),
            Resolution::Declared {
                file,
                name,
                line,
                route,
            } => (file, name, line, route),
            _ => return None,
        },
        // `module.name`: find the module, then the name inside it.
        Some((module_reference, attribute)) => {
            let (module_path, how) = module_file(modules, from, module_reference)?;
            let module = modules.file(&module_path)?;
            match modules.resolve_python(module, attribute) {
                Resolution::Local { line } => (module_path, attribute.to_owned(), line, how),
                Resolution::Declared {
                    file,
                    name,
                    line,
                    route,
                } => (file, name, line, format!("{how} then {route}")),
                _ => return None,
            }
        }
    })
}

/// The file a module reference in `from` names, and how it was bound.
///
/// Exactly one binding must lead to exactly one file. Two imports that both
/// explain the reference, or a specifier matching several files, leave the
/// question open and the answer is `None`.
fn module_file(
    modules: &ModuleIndex<'_>,
    from: &FileAnalysis,
    reference: &str,
) -> Option<(String, String)> {
    let (head, tail) = reference
        .split_once('.')
        .map_or((reference, ""), |(head, tail)| (head, tail));
    let with_tail = |specifier: String| {
        if tail.is_empty() {
            specifier
        } else {
            join_specifier(&specifier, tail)
        }
    };

    let mut found: Vec<(String, String)> = Vec::new();
    for import in &from.imports {
        let mut candidates: Vec<(String, String)> = Vec::new();

        // `import shop.views` — the reference is the specifier itself.
        if import.named.is_empty()
            && import.namespace_name.is_none()
            && import.specifier == reference
        {
            candidates.push((
                import.specifier.clone(),
                format!("`import {}`", import.specifier),
            ));
        }
        // `import shop.views as v`
        if import.namespace_name.as_deref() == Some(head) && head != "*" {
            candidates.push((
                with_tail(import.specifier.clone()),
                format!("`import {} as {head}`", import.specifier),
            ));
        }
        // `from shop import views`, `from . import views as v`
        for named in &import.named {
            if named.local.as_deref().unwrap_or(named.imported.as_str()) == head {
                candidates.push((
                    with_tail(join_specifier(&import.specifier, &named.imported)),
                    format!("`from {} import {}`", import.specifier, named.imported),
                ));
            }
        }

        for (specifier, how) in candidates {
            if let Some(path) = modules.resolve_python_module(&specifier, &from.path) {
                if !found.iter().any(|(known, _)| *known == path) {
                    found.push((path, how));
                }
            }
        }
    }
    match found.len() {
        1 => found.pop(),
        _ => None,
    }
}

/// `shop` + `views` is `shop.views`; `.` + `views` is `.views`.
fn join_specifier(specifier: &str, name: &str) -> String {
    if specifier.ends_with('.') || specifier.is_empty() {
        format!("{specifier}{name}")
    } else {
        format!("{specifier}.{name}")
    }
}

/// `a`, `a.b`, `a.b.c` — identifiers joined by dots, and nothing else.
fn is_dotted_name(text: &str) -> bool {
    !text.is_empty()
        && text.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}
