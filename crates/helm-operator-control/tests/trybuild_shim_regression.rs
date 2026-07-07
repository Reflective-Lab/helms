//! Trybuild regression suite for the RFL-154 seam cut (helm-operator-control).
//!
//! Guards that the two dropped dep-edges (workbench-backend, prio-agent-ops)
//! cannot be re-introduced without a compile failure. Each `.rs` file in
//! `tests/compile_fail/` must fail to compile.
//!
//! Run `TRYBUILD=overwrite cargo test -p helm-operator-control \
//! --test trybuild_shim_regression` to regenerate stderr snapshots.

#[test]
fn shim_regression_guards() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
