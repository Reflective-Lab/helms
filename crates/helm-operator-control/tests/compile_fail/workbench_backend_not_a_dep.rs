//! Regression guard (RFL-154): workbench_backend is NOT a dependency of
//! helm-operator-control after the seam cut.
//!
//! Before T5a, operator-control depended on workbench-backend for
//! OperatorControlPreview. That type now lives in
//! `helm_module_contracts::operator_preview`. This file must NOT compile,
//! proving the old dep-edge is gone and cannot be accidentally re-introduced.

fn main() {
    // workbench_backend is not in the dep tree — this crate path is not resolvable.
    let _: workbench_backend::views::OperatorControlPreview;
}
