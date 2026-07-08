// ── Compile-fail guard: InMemoryLeaseStore / InMemoryEventLog require `--features memory` ──
//
// trybuild cannot control feature flags per test case — features are resolved
// package-wide at `cargo test` invocation time.  Because the gate command is
// `cargo test -p helm-event-substrate --features memory,sse`, a trybuild
// compile_fail case that imports `InMemoryLeaseStore` would succeed (not fail)
// and thus the `compile_fail` expectation itself would fail.
//
// The documented fallback: run `cargo check --no-default-features` as a subprocess
// and assert it exits cleanly.  The actual type-system guarantee is the
// `#[cfg(feature = "memory")]` gate in `src/memory.rs`; this test validates
// that the gate does not break the crate and that the crate surface is clean
// without ANY feature enabled.

#[test]
fn memory_feature_gate_cargo_check_without_features() {
    // `CARGO_MANIFEST_DIR` is set at compile time to the crate directory.
    // Running `cargo check -p helm-event-substrate --no-default-features` from
    // that directory lets cargo locate the workspace and build only this crate.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let output = std::process::Command::new("cargo")
        .args([
            "check",
            "-p",
            "helm-event-substrate",
            "--no-default-features",
            "--quiet",
        ])
        .current_dir(manifest_dir)
        .output()
        .expect("failed to spawn `cargo check`");

    assert!(
        output.status.success(),
        "`cargo check -p helm-event-substrate --no-default-features` failed.\n\
         This means the crate has a compile error when `memory` and `sse` features\n\
         are both absent — the feature gate is broken.\n\
         stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
