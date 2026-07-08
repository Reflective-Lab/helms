//! `TruthKey` — typed, parse-don't-validate newtype for truth identifiers.
//!
//! # Rationale
//!
//! Truth keys are runtime string values that cross HTTP boundaries and serve as
//! lookup tokens in the `TruthCatalog`. Passing raw `&str` throughout the API
//! pushes validation responsibility to every call-site, leading to either
//! defensive panics or silent "no match" bugs.
//!
//! `TruthKey` encodes the invariant once at the parse boundary (HTTP handler,
//! CLI arg, deserialization) and allows the interior of the system to handle a
//! value that is **guaranteed** to be a valid kebab-case identifier. This is
//! the parse-don't-validate pattern applied to a string type.
//!
//! # Grammar
//!
//! A valid `TruthKey` is a kebab-case identifier:
//!
//! ```text
//! truth-key ::= segment ('-' segment)*
//! segment   ::= [a-z0-9]+
//! ```
//!
//! In other words:
//! - One or more lowercase ASCII alphanumeric segments.
//! - Segments are joined by exactly one hyphen.
//! - No leading or trailing hyphens.
//! - No consecutive hyphens.
//! - Non-ASCII characters are rejected.
//! - Empty strings are rejected.
//!
//! This matches the output of the `slug()` helper that produced the `&'static str`
//! truth keys used by `TruthDefinition::key` throughout this crate.

use std::fmt;
use std::str::FromStr;

use serde::Serialize;
use thiserror::Error;

/// A validated, kebab-case truth identifier.
///
/// Construct via [`TruthKey::parse`] or via the [`FromStr`] impl. Both reject
/// any string that does not conform to the grammar; see the module-level docs
/// for the full grammar.
///
/// Use [`TruthKey::as_str`] to borrow the inner value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct TruthKey(String);

/// Returned when a string cannot be parsed as a [`TruthKey`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid truth key {input:?}: {reason}")]
pub struct InvalidTruthKey {
    /// The original string that was rejected.
    pub input: String,
    /// A human-readable description of why the string was rejected.
    pub reason: &'static str,
}

impl TruthKey {
    /// Parse `s` as a kebab-case truth key.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidTruthKey`] when `s` is empty, contains non-ASCII
    /// characters, uses uppercase letters, has a leading or trailing hyphen, or
    /// has consecutive hyphens.
    pub fn parse(s: &str) -> Result<Self, InvalidTruthKey> {
        let err = |reason| InvalidTruthKey {
            input: s.to_owned(),
            reason,
        };

        if s.is_empty() {
            return Err(err("must not be empty"));
        }

        if !s.is_ascii() {
            return Err(err("must contain only ASCII characters"));
        }

        if s.starts_with('-') {
            return Err(err("must not start with a hyphen"));
        }

        if s.ends_with('-') {
            return Err(err("must not end with a hyphen"));
        }

        for ch in s.chars() {
            if !matches!(ch, 'a'..='z' | '0'..='9' | '-') {
                return Err(err(
                    "must contain only lowercase ASCII letters, digits, and hyphens",
                ));
            }
        }

        if s.contains("--") {
            return Err(err("must not contain consecutive hyphens"));
        }

        Ok(TruthKey(s.to_owned()))
    }

    /// Borrow the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for TruthKey {
    type Err = InvalidTruthKey;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for TruthKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for TruthKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{InvalidTruthKey, TruthKey};

    fn valid(s: &str) -> TruthKey {
        TruthKey::parse(s).expect("expected valid key")
    }

    fn invalid(s: &str) -> InvalidTruthKey {
        TruthKey::parse(s).expect_err("expected invalid key")
    }

    // --- valid cases ---

    #[test]
    fn single_segment_is_valid() {
        assert_eq!(valid("lead").as_str(), "lead");
    }

    #[test]
    fn multi_segment_is_valid() {
        assert_eq!(
            valid("qualify-inbound-lead").as_str(),
            "qualify-inbound-lead"
        );
    }

    #[test]
    fn segment_with_digits_is_valid() {
        assert_eq!(valid("truth-42-abc").as_str(), "truth-42-abc");
    }

    #[test]
    fn display_roundtrips() {
        let key = valid("score-inbound-fit");
        assert_eq!(key.to_string(), "score-inbound-fit");
    }

    #[test]
    fn fromstr_roundtrips() {
        let key: TruthKey = "plan-outbound-campaign".parse().unwrap();
        assert_eq!(key.as_str(), "plan-outbound-campaign");
    }

    // --- invalid cases ---

    #[test]
    fn empty_string_is_invalid() {
        let e = invalid("");
        assert_eq!(e.input, "");
        assert!(e.reason.contains("empty"), "reason: {}", e.reason);
    }

    #[test]
    fn uppercase_is_invalid() {
        let e = invalid("Qualify");
        assert!(
            e.reason.contains("lowercase"),
            "reason should mention lowercase: {}",
            e.reason
        );
    }

    #[test]
    fn underscore_is_invalid() {
        let e = invalid("submit_expense");
        assert!(
            e.reason.contains("lowercase"),
            "reason: {}",
            e.reason
        );
    }

    #[test]
    fn leading_hyphen_is_invalid() {
        let e = invalid("-lead");
        assert!(
            e.reason.contains("start"),
            "reason: {}",
            e.reason
        );
    }

    #[test]
    fn trailing_hyphen_is_invalid() {
        let e = invalid("lead-");
        assert!(
            e.reason.contains("end"),
            "reason: {}",
            e.reason
        );
    }

    #[test]
    fn non_ascii_is_invalid() {
        let e = invalid("lead-über");
        assert!(
            e.reason.contains("ASCII"),
            "reason: {}",
            e.reason
        );
    }

    #[test]
    fn consecutive_hyphens_invalid() {
        let e = invalid("lead--inbound");
        assert!(
            e.reason.contains("consecutive"),
            "reason: {}",
            e.reason
        );
    }
}
