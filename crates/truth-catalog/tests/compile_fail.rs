//! Compile-fail regression guards for truth-catalog seam (RFL-172 T7).
//!
//! Verifies that certain type-system seam properties hold at compile time.
#[test]
fn compile_fail_guards() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
