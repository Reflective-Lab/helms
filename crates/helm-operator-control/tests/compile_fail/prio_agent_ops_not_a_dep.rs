//! Regression guard (RFL-154): prio_agent_ops is NOT a dependency of
//! helm-operator-control after the seam cut.
//!
//! Before T3/T5a, operator-control imported 18 vocabulary types from
//! prio-agent-ops (including JobReadinessPacket). Those types now live in
//! `helm_module_contracts::operator_receipts`. This file must NOT compile,
//! proving the old dep-edge is gone and cannot be accidentally re-introduced.

fn main() {
    // prio_agent_ops is not in the dep tree — this crate path is not resolvable.
    let _: prio_agent_ops::JobReadinessPacket;
}
