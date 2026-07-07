//! Compile-fail suite for `helm-module-contracts` (RFL-154 T7).
//!
//! Each `.rs` file under `tests/compile_fail/` must fail to compile.
//! The expected compiler output is captured in the matching `.stderr` file
//! alongside each case. Run with `TRYBUILD=overwrite cargo test
//! -p helm-module-contracts --test trybuild_compile_fail` to regenerate
//! snapshots after intentional API changes.

#[test]
fn compile_fail_cases() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
