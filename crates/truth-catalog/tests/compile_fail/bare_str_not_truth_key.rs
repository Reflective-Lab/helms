//! Guard: bare &str cannot be coerced to &TruthKey.
fn main() {
    let _: &truth_catalog::TruthKey = "qualify-inbound-lead";
}
