//! Tracing a component through a client wrapper to the request it issues.

use cartograph_core::EdgeKind;
use cartograph_graph::ArchitectureGraph;
use cartograph_parser::Analyzer;
use cartograph_parser::model::FileAnalysis;
use cartograph_resolver::{ModuleIndex, Resolution, add_client_call_edges};

fn analyze(files: &[(&str, &str)]) -> Vec<FileAnalysis> {
    let mut a = Analyzer::new().expect("grammars load");
    files
        .iter()
        .map(|(p, s)| a.analyze_source(p, s).expect("fixture parses"))
        .collect()
}

/// Call edges as `(caller, callee)` pairs.
fn call_edges(files: &[(&str, &str)]) -> Vec<(String, String)> {
    let parsed = analyze(files);
    let modules = ModuleIndex::build(&parsed);
    let mut graph = ArchitectureGraph::new();
    add_client_call_edges(&mut graph, &parsed, &modules, None).expect("endpoints exist");
    let mut out: Vec<(String, String)> = graph
        .edges()
        .filter(|e| e.kind() == EdgeKind::Call)
        .map(|e| {
            (
                graph.node(e.source()).unwrap().name().to_owned(),
                graph.node(e.target()).unwrap().name().to_owned(),
            )
        })
        .collect();
    out.sort();
    out
}

const SERVICES: &str = r"
export class PoolService {
    public static postPool(data) {
        return __request(OpenAPI, { method: 'POST', url: '/api/v2/pools' });
    }
}
";

// ── Positive ────────────────────────────────────────────────────────

#[test]
fn a_caller_reaches_the_function_that_issues_the_request() {
    let edges = call_edges(&[
        ("ui/gen/services.gen.ts", SERVICES),
        (
            "ui/src/useAddPool.ts",
            r#"
import { PoolService } from "../gen/services.gen";

export const useAddPool = () => PoolService.postPool({});
"#,
        ),
    ]);
    assert!(
        edges.contains(&("useAddPool".to_owned(), "postPool".to_owned())),
        "{edges:?}"
    );
}

/// Airflow: component → hook → generated hook → generated client.
#[test]
fn a_call_chain_is_followed_through_intermediate_layers() {
    let edges = call_edges(&[
        ("ui/gen/services.gen.ts", SERVICES),
        (
            "ui/gen/queries.ts",
            r#"
import { PoolService } from "./services.gen";

export const usePoolServicePostPool = (options) =>
  useMutation({ mutationFn: ({ body }) => PoolService.postPool({ body }) });
"#,
        ),
        (
            "ui/src/queries/useAddPool.ts",
            r#"
import { usePoolServicePostPool } from "../../gen/queries";

export const useAddPool = () => usePoolServicePostPool({});
"#,
        ),
        (
            "ui/src/pages/AddPoolButton.tsx",
            r#"
import { useAddPool } from "src/queries/useAddPool";

export const AddPoolButton = () => {
  const add = useAddPool();
  return add;
};
"#,
        ),
    ]);
    // Every hop of the chain, and nothing invented between them.
    assert!(
        edges.contains(&("useAddPool".to_owned(), "usePoolServicePostPool".to_owned())),
        "{edges:?}"
    );
    assert!(
        edges.contains(&("usePoolServicePostPool".to_owned(), "postPool".to_owned())),
        "{edges:?}"
    );
}

/// Airflow's `openapi-gen/queries/index.ts` is two wildcard re-exports.
#[test]
fn a_barrel_file_is_followed_to_the_declaration() {
    let edges = call_edges(&[
        ("ui/gen/services.gen.ts", SERVICES),
        ("ui/gen/index.ts", "export * from \"./services.gen\";\n"),
        (
            "ui/src/useAddPool.ts",
            r#"
import { PoolService } from "../gen";

export const useAddPool = () => PoolService.postPool({});
"#,
        ),
    ]);
    assert!(
        edges.contains(&("useAddPool".to_owned(), "postPool".to_owned())),
        "a barrel re-export must not end the trail: {edges:?}"
    );
}

/// Airflow's `vite.config.ts`: `resolve: { alias: { openapi: "/openapi-gen" } }`.
#[test]
fn a_build_configuration_alias_is_followed() {
    let edges = call_edges(&[
        (
            "ui/vite.config.ts",
            "export default defineConfig({ resolve: { alias: { openapi: \"/gen\" } } });\n",
        ),
        ("ui/gen/services.gen.ts", SERVICES),
        (
            "ui/src/useAddPool.ts",
            r#"
import { PoolService } from "openapi/services.gen";

export const useAddPool = () => PoolService.postPool({});
"#,
        ),
    ]);
    assert!(
        edges.contains(&("useAddPool".to_owned(), "postPool".to_owned())),
        "a declared alias must be followed: {edges:?}"
    );
}

// ── Negative ────────────────────────────────────────────────────────

#[test]
fn a_call_that_reaches_no_request_produces_no_edge() {
    // This is not a general call graph. A call is only interesting if what it
    // calls eventually issues a request.
    let edges = call_edges(&[
        (
            "ui/src/format.ts",
            "export const format = (s) => s.trim();\n",
        ),
        (
            "ui/src/page.tsx",
            r#"
import { format } from "./format";

export const Page = () => format("x");
"#,
        ),
    ]);
    assert!(
        edges.is_empty(),
        "an unrelated call became an edge: {edges:?}"
    );
}

#[test]
fn an_unaliased_package_import_is_not_followed() {
    let edges = call_edges(&[
        ("ui/gen/services.gen.ts", SERVICES),
        (
            "ui/src/useAddPool.ts",
            r#"
import { PoolService } from "@some/package";

export const useAddPool = () => PoolService.postPool({});
"#,
        ),
    ]);
    assert!(
        edges.is_empty(),
        "a package specifier must not resolve to a project file: {edges:?}"
    );
}

#[test]
fn a_same_named_function_in_another_module_is_not_the_target() {
    // Two modules declaring `postPool`; only the imported one is called.
    let edges = call_edges(&[
        ("ui/gen/services.gen.ts", SERVICES),
        (
            "ui/other/services.gen.ts",
            r"
export class PoolService {
    public static postPool(data) {
        return __request(OpenAPI, { method: 'POST', url: '/other/pools' });
    }
}
",
        ),
        (
            "ui/src/useAddPool.ts",
            r#"
import { PoolService } from "../gen/services.gen";

export const useAddPool = () => PoolService.postPool({});
"#,
        ),
    ]);
    assert_eq!(edges.len(), 1, "{edges:?}");
}

// ── Ambiguity ───────────────────────────────────────────────────────

#[test]
fn two_barrels_exporting_one_name_are_ambiguous() {
    let parsed = analyze(&[
        ("ui/a.ts", "export const thing = 1;\n"),
        ("ui/b.ts", "export const thing = 2;\n"),
        (
            "ui/index.ts",
            "export * from \"./a\";\nexport * from \"./b\";\n",
        ),
        ("ui/use.ts", "import { thing } from \"./index\";\n"),
    ]);
    let modules = ModuleIndex::build(&parsed);
    let from = modules.file("ui/use.ts").expect("analysed");
    let resolution = modules.resolve_typescript(from, "thing");
    assert!(
        matches!(resolution, Resolution::Ambiguous { .. }),
        "two barrels exporting one name cannot resolve to one: {resolution:?}"
    );
    assert!(!resolution.is_definite());
}

#[test]
fn an_alias_only_governs_files_beneath_its_configuration() {
    // Airflow declares five Vite configurations. An alias named `openapi` in
    // one frontend must not resolve an import in another.
    let parsed = analyze(&[
        (
            "apps/a/vite.config.ts",
            "export default defineConfig({ resolve: { alias: { openapi: \"/gen\" } } });\n",
        ),
        (
            "apps/a/gen/client.ts",
            "export const call = () => fetch(\"/a\");\n",
        ),
        (
            "apps/b/src/use.ts",
            "import { call } from \"openapi/client\";\n",
        ),
    ]);
    let modules = ModuleIndex::build(&parsed);
    let from = modules.file("apps/b/src/use.ts").expect("analysed");
    let resolution = modules.resolve_typescript(from, "call");
    assert!(
        !resolution.is_definite(),
        "an alias from another project must not resolve here: {resolution:?}"
    );
}

// ── Evidence ────────────────────────────────────────────────────────

#[test]
fn every_call_edge_says_why_it_exists() {
    let parsed = analyze(&[
        ("ui/gen/services.gen.ts", SERVICES),
        (
            "ui/src/useAddPool.ts",
            "import { PoolService } from \"../gen/services.gen\";\nexport const useAddPool = () => PoolService.postPool({});\n",
        ),
    ]);
    let modules = ModuleIndex::build(&parsed);
    let mut graph = ArchitectureGraph::new();
    add_client_call_edges(&mut graph, &parsed, &modules, None).expect("endpoints exist");
    for edge in graph.edges().filter(|e| e.kind() == EdgeKind::Call) {
        let evidence = edge.evidence().as_str();
        assert!(evidence.contains("issues an HTTP request"), "{evidence}");
        assert!(edge.confidence().get() > 0.0);
        assert!(!edge.location().file().is_empty());
    }
}
