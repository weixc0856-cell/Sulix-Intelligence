//! Architecture dependency guard — decoupling plan P7.
//!
//! Reads the workspace's *declared* normal (non-dev / non-build) dependency
//! edges from `cargo metadata --no-deps` (no build, no network) and asserts the
//! DDD layering `Delivery → Application → Domain ↑ Ports ↑ Infrastructure`
//! (see `docs/architecture/final-architecture-v2.md` and
//! `docs/superpowers/plans/2026-09-05-decoupling-advance.md` §4):
//!
//!   1. **Domain** crates never depend on a host (`worker`) or concrete-infra
//!      crate (`store`/`vectorize`/`embedding`/`event-store`/`object-store`/
//!      `infrastructure`).  Hard rule — already clean.
//!   2. **application** never depends on `worker` or a concrete-infra crate.
//!      The `store` edge was removed when application re-pointed to the
//!      infra-free `domain` ports (`store` is only a dev-dependency for
//!      `MemoryStore` in tests, which this normal-deps guard ignores).
//!   3. **api** never depends on a concrete-infra crate (delivery may keep
//!      `worker`).  Phase 2 (Domain Lift) cut all six `api:*` edges — the API
//!      reaches services only through `composition::ProductionAppServices`.
//!   4. **domain** and **application** never depend on the `composition` crate
//!      (wiring-only).  Prevents `domain → composition → application →
//!      domain`-style reverse cycles from being introduced.
//!   5. The workspace crate graph contains no cycles.
//!
//! `GRANDFATHERED` is empty: every forbidden edge is now hard-enforced (same
//! idiom as `scripts/check-layered-deps.sh`).  A new forbidden edge fails
//! immediately; there are no legacy exceptions left to remove.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Layer taxonomy — keep in sync with final-architecture-v2.md §4.
// ---------------------------------------------------------------------------

/// Pure Domain layer: aggregates / VOs / domain services. No host access.
const DOMAIN_CRATES: &[&str] = &[
    "claim-engine",
    "decision-engine",
    "domain",
    "events",
    "intelligence-domain",
    "model-runtime",
    "reasoning-framework",
    "reflection-engine",
    "shared-kernel",
    "signal-engine",
];

/// Application layer — the unique use-case entry point (generic over narrow
/// `domain` subtraits; concrete stores only via `composition` / dev-deps).
const APPLICATION_CRATES: &[&str] = &["application"];

/// Delivery layer crates governed by the concrete-infra rule. `worker` stays
/// allowed here (delivery is the Cloudflare host).
const DELIVERY_CRATES: &[&str] = &["api"];

/// Wiring-only composition crate (aliases `AppServices<D1Store>`); nothing may
/// depend on it except the delivery layer.
const COMPOSITION_CRATES: &[&str] = &["composition"];

/// Host / Cloudflare-sdk crate (external, not a workspace member).
const HOST_CRATES: &[&str] = &["worker"];

/// Concrete-infrastructure crates (workspace members).
const INFRA_CRATES: &[&str] = &["store", "vectorize", "embedding", "event-store", "object-store", "infrastructure"];

/// Forbidden edges still present today — none. `GRANDFATHERED` is empty and
/// every forbidden edge is hard-enforced. Format `source:target`; the moment
/// an edge disappears it would be reported as removable, so keep this empty.
const GRANDFATHERED: &[&str] = &[];

// ---------------------------------------------------------------------------
// cargo metadata (--no-deps) JSON surface.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    /// `source == null` marks workspace members (path packages).
    source: Option<String>,
    #[serde(default)]
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    /// `null` = normal; `"dev"` = dev-dependency; `"build"` = build-dependency.
    #[serde(default)]
    kind: Option<String>,
}

/// Workspace root = the ancestor of `CARGO_MANIFEST_DIR` whose Cargo.toml
/// declares `[workspace]`.
fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            let text =
                std::fs::read_to_string(&manifest).unwrap_or_else(|e| panic!("read {}: {e}", manifest.display()));
            if text.lines().any(|line| line.trim() == "[workspace]") {
                return dir;
            }
        }
        if !dir.pop() {
            panic!("could not locate workspace root above {}", env!("CARGO_MANIFEST_DIR"));
        }
    }
}

fn load_declared_deps() -> BTreeMap<String, BTreeSet<String>> {
    let root = workspace_root();
    let out = Command::new(env!("CARGO"))
        .arg("metadata")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .arg("--no-deps")
        .arg("--format-version")
        .arg("1")
        .output()
        .expect("failed to spawn `cargo metadata`");

    assert!(out.status.success(), "`cargo metadata` failed:\n{}", String::from_utf8_lossy(&out.stderr));

    let meta: Metadata = serde_json::from_slice(&out.stdout).expect("parse `cargo metadata` JSON output");

    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for pkg in meta.packages {
        if pkg.source.is_some() {
            continue; // registry/git dep — only workspace members are governed.
        }
        let normal: BTreeSet<String> = pkg
            .dependencies
            .iter()
            .filter(|d| d.kind.is_none()) // normal deps only — dev/test edges excluded
            .map(|d| d.name.clone())
            .collect();
        graph.insert(pkg.name, normal);
    }
    graph
}

fn assert_names_resolve(graph: &BTreeMap<String, BTreeSet<String>>) {
    for &name in DOMAIN_CRATES
        .iter()
        .chain(APPLICATION_CRATES)
        .chain(DELIVERY_CRATES)
        .chain(COMPOSITION_CRATES)
        .chain(INFRA_CRATES)
    {
        assert!(graph.contains_key(name), "architecture guard names crate `{name}` which is not a workspace member");
    }
}

/// Collects every violation of a rule in `report`.
fn assert_blocked(
    graph: &BTreeMap<String, BTreeSet<String>>,
    source: &str,
    targets: &[&str],
    grandfathered: &[&str],
    report: &mut Vec<String>,
) {
    for &target in targets {
        let key = format!("{source}:{target}");
        if graph.get(source).is_some_and(|deps| deps.contains(target)) && !grandfathered.contains(&key.as_str()) {
            report.push(format!("✗ forbidden edge: `{source}` depends on `{target}` (not grandfathered)"));
        }
    }
}

/// Reports grandfather entries that are no longer present → hard-enforce them.
fn report_removable(graph: &BTreeMap<String, BTreeSet<String>>, hints: &mut Vec<String>) {
    for entry in GRANDFATHERED {
        let (source, target) = entry.split_once(':').expect("GRANDFATHERED must be `source:target`");
        let present = graph.get(source).is_some_and(|deps| deps.contains(target));
        if !present {
            hints.push(format!(
                "✓ removable: `{source}:{target}` is gone — delete it from GRANDFATHERED to hard-enforce"
            ));
        }
    }
}

/// Returns a sample cycle path if the (normal-dep) crate graph has one.
fn find_cycle(graph: &BTreeMap<String, BTreeSet<String>>) -> Option<Vec<String>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }
    fn visit(
        node: &str,
        graph: &BTreeMap<String, BTreeSet<String>>,
        color: &mut BTreeMap<String, Color>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        color.insert(node.to_string(), Color::Gray);
        path.push(node.to_string());
        for next in &graph[node] {
            if !graph.contains_key(next) {
                continue; // external (registry) dep — cycles only span workspace crates
            }
            match color.get(next).copied().unwrap_or(Color::White) {
                Color::Gray => {
                    let start = path.iter().position(|n| n == next).unwrap_or(0);
                    let mut cycle = path[start..].to_vec();
                    cycle.push(next.clone());
                    return Some(cycle);
                }
                Color::White => {
                    if let Some(cycle) = visit(next, graph, color, path) {
                        return Some(cycle);
                    }
                }
                Color::Black => {}
            }
        }
        color.insert(node.to_string(), Color::Black);
        path.pop();
        None
    }

    let mut color: BTreeMap<String, Color> = graph.keys().map(|n| (n.clone(), Color::White)).collect();
    let mut path = Vec::new();
    for node in graph.keys() {
        if color[node] == Color::White {
            if let Some(cycle) = visit(node, graph, &mut color, &mut path) {
                return Some(cycle);
            }
        }
    }
    None
}

#[test]
fn declared_dependencies_respect_ddd_layering() {
    let graph = load_declared_deps();
    assert_names_resolve(&graph);

    let mut report: Vec<String> = Vec::new();

    // 1. Domain → {worker, concrete infra}: forbidden outright (none today).
    for &crate_name in DOMAIN_CRATES {
        assert_blocked(&graph, crate_name, HOST_CRATES, &[], &mut report);
        assert_blocked(&graph, crate_name, INFRA_CRATES, &[], &mut report);
    }

    // 2. application → {worker, concrete infra}: forbidden outright.
    for &crate_name in APPLICATION_CRATES {
        assert_blocked(&graph, crate_name, HOST_CRATES, &[], &mut report);
        assert_blocked(&graph, crate_name, INFRA_CRATES, GRANDFATHERED, &mut report);
    }

    // 3. api → concrete infra (delivery may keep `worker`): forbidden outright.
    for &crate_name in DELIVERY_CRATES {
        assert_blocked(&graph, crate_name, INFRA_CRATES, GRANDFATHERED, &mut report);
    }

    // 4. domain + application → composition (wiring-only): forbidden outright.
    for &crate_name in DOMAIN_CRATES.iter().chain(APPLICATION_CRATES) {
        assert_blocked(&graph, crate_name, COMPOSITION_CRATES, &[], &mut report);
    }

    // 5. No cycles.
    if let Some(cycle) = find_cycle(&graph) {
        report.push(format!("✗ dependency cycle: {}", cycle.join(" → ")));
    }

    let mut hints: Vec<String> = Vec::new();
    report_removable(&graph, &mut hints);
    for hint in &hints {
        eprintln!("{hint}");
    }

    assert!(report.is_empty(), "architecture dependency guard FAILED:\n{}", report.join("\n"));
}
