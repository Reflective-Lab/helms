//! Compile-fail gate: private validation helpers are not part of the public API.
//!
//! `validate_sha256` (and its siblings) are `fn` — module-private. External
//! callers MUST go through `JobReadinessPacket::new()` or
//! `OperatorLedgerEntry::new()` to get a validated value. This file must not
//! compile, enforcing the parse-don't-validate contract at the type level.

fn main() {
    // validate_sha256 is private — calling it directly is a compile error.
    // Callers must use JobReadinessPacket::new() / OperatorLedgerEntry::new().
    let _ = helm_module_contracts::operator_receipts::validate_sha256(
        "payload_hash",
        "sha256:abc",
    );
}
