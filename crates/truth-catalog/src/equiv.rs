//! IntentPacket equivalence checking for the organism 1.8.0 migration.
//!
//! Compares two `IntentPacket`s field-by-field per the rules from the
//! 2026-05-07 organism handoff:
//! - `id` is excluded (random per packet)
//! - `expires` round-trips through RFC-3339
//! - `constraints`, `authority`, `forbidden` treated as sets (order-independent)
//! - `context` compared via `serde_json::Value`
//!
//! Used during step 2 of the migration to prove that an axiom-compiled
//! IntentPacket matches the legacy `organism_recipe` output before the recipe
//! is deleted.

use std::collections::HashSet;

use organism_pack::{ForbiddenAction, IntentPacket};

/// A single field-level difference between two IntentPackets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDiff {
    pub field: &'static str,
    pub left: String,
    pub right: String,
}

/// All differences found between two IntentPackets.
#[derive(Debug, Clone, Default)]
pub struct Diff {
    pub fields: Vec<FieldDiff>,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

impl std::fmt::Display for Diff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.fields.is_empty() {
            return write!(f, "<no diff>");
        }
        for d in &self.fields {
            writeln!(f, "  {}:", d.field)?;
            writeln!(f, "    left:  {}", d.left)?;
            writeln!(f, "    right: {}", d.right)?;
        }
        Ok(())
    }
}

/// Compare two IntentPackets per the migration equivalence rules.
///
/// Returns `Ok(())` when packets agree on every field except `id`.
/// Returns `Err(Diff)` listing every disagreement.
pub fn intent_packet_equiv(left: &IntentPacket, right: &IntentPacket) -> Result<(), Diff> {
    let mut diff = Diff::default();

    if left.outcome != right.outcome {
        diff.fields.push(FieldDiff {
            field: "outcome",
            left: left.outcome.clone(),
            right: right.outcome.clone(),
        });
    }
    if left.context != right.context {
        diff.fields.push(FieldDiff {
            field: "context",
            left: left.context.to_string(),
            right: right.context.to_string(),
        });
    }
    if !str_sets_equal(&left.constraints, &right.constraints) {
        diff.fields.push(FieldDiff {
            field: "constraints",
            left: format!("{:?}", left.constraints),
            right: format!("{:?}", right.constraints),
        });
    }
    if !str_sets_equal(&left.authority, &right.authority) {
        diff.fields.push(FieldDiff {
            field: "authority",
            left: format!("{:?}", left.authority),
            right: format!("{:?}", right.authority),
        });
    }
    if !forbidden_sets_equal(&left.forbidden, &right.forbidden) {
        diff.fields.push(FieldDiff {
            field: "forbidden",
            left: format!("{:?}", left.forbidden),
            right: format!("{:?}", right.forbidden),
        });
    }
    if left.reversibility != right.reversibility {
        diff.fields.push(FieldDiff {
            field: "reversibility",
            left: format!("{:?}", left.reversibility),
            right: format!("{:?}", right.reversibility),
        });
    }
    if left.expires.to_rfc3339() != right.expires.to_rfc3339() {
        diff.fields.push(FieldDiff {
            field: "expires",
            left: left.expires.to_rfc3339(),
            right: right.expires.to_rfc3339(),
        });
    }
    if left.expiry_action != right.expiry_action {
        diff.fields.push(FieldDiff {
            field: "expiry_action",
            left: format!("{:?}", left.expiry_action),
            right: format!("{:?}", right.expiry_action),
        });
    }

    if diff.is_empty() { Ok(()) } else { Err(diff) }
}

fn str_sets_equal(a: &[String], b: &[String]) -> bool {
    let a: HashSet<&str> = a.iter().map(String::as_str).collect();
    let b: HashSet<&str> = b.iter().map(String::as_str).collect();
    a == b
}

fn forbidden_sets_equal(a: &[ForbiddenAction], b: &[ForbiddenAction]) -> bool {
    let a: HashSet<(&str, &str)> = a
        .iter()
        .map(|f| (f.action.as_str(), f.reason.as_str()))
        .collect();
    let b: HashSet<(&str, &str)> = b
        .iter()
        .map(|f| (f.action.as_str(), f.reason.as_str()))
        .collect();
    a == b
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn packet(outcome: &str) -> IntentPacket {
        IntentPacket::new(outcome, Utc::now() + Duration::hours(1))
    }

    #[test]
    fn identical_packets_are_equivalent() {
        let a = packet("ship the thing");
        let mut b = a.clone();
        b.id = uuid::Uuid::new_v4(); // id is excluded
        assert!(intent_packet_equiv(&a, &b).is_ok());
    }

    #[test]
    fn outcome_diff_reported() {
        let a = packet("a");
        let mut b = packet("b");
        b.expires = a.expires;
        let diff = intent_packet_equiv(&a, &b).unwrap_err();
        assert_eq!(diff.fields.len(), 1);
        assert_eq!(diff.fields[0].field, "outcome");
    }

    #[test]
    fn constraint_order_does_not_matter() {
        let mut a = packet("x");
        let mut b = packet("x");
        a.constraints = vec!["one".into(), "two".into()];
        b.constraints = vec!["two".into(), "one".into()];
        b.expires = a.expires;
        assert!(intent_packet_equiv(&a, &b).is_ok());
    }
}
