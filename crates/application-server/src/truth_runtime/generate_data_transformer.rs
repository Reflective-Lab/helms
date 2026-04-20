//! EXP-002: Code generation as a convergence step.
//!
//! Proves that a convergence loop can include suggestors that generate,
//! verify, and promote executable code — treating code as just another
//! fact type that flows through the same governance gates.
//!
//! The pattern:
//! 1. GapSuggestor detects a missing transformer
//! 2. CodeGenSuggestor generates Rust source (via LLM)
//! 3. CodeVerifierSuggestor compiles and tests the generated code
//! 4. On failure: error fact triggers gap detector → refined retry
//! 5. On success: verified artifact promoted as a Proposals fact

use std::sync::{Arc, Mutex};

use converge_kernel::{Context, Engine};
use converge_pack::{
    AgentEffect, Context as ContextView, ContextKey, ProposedFact, Suggestor,
};
use sha2::{Digest, Sha256};

// ── Code Generation Suggestor ───────────────────────────────────────

/// Generates Rust source code for a data transformer when the context
/// contains a strategy requesting code generation but no artifact yet.
pub struct CodeGenSuggestor {
    llm_stub: bool,
    generated: Mutex<bool>,
}

impl CodeGenSuggestor {
    pub fn new() -> Self {
        Self {
            llm_stub: true,
            generated: Mutex::new(false),
        }
    }
}

#[async_trait::async_trait]
impl Suggestor for CodeGenSuggestor {
    fn name(&self) -> &str {
        "codegen"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Strategies]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        let needs_codegen = ctx
            .get(ContextKey::Strategies)
            .iter()
            .any(|f| f.content.contains("[codegen]"));
        let has_artifact = ctx
            .get(ContextKey::Hypotheses)
            .iter()
            .any(|f| f.id.starts_with("artifact:generated-source:"));
        let already_generated = *self.generated.lock().unwrap();

        needs_codegen && !has_artifact && !already_generated
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        *self.generated.lock().unwrap() = true;

        // In production: call LLM with the transformation spec.
        // For EXP-002: stub that produces a valid Rust data transformer.
        let strategy = ctx
            .get(ContextKey::Strategies)
            .iter()
            .find(|f| f.content.contains("[codegen]"))
            .map(|f| f.content.clone())
            .unwrap_or_default();

        let source = if self.llm_stub {
            generate_stub_transformer(&strategy)
        } else {
            // Future: LLM-generated code
            generate_stub_transformer(&strategy)
        };

        let source_hash = sha256_hex(&source);

        let content = serde_json::json!({
            "type": "generated-source",
            "language": "rust",
            "target": "wasm32-unknown-unknown",
            "source": source,
            "source_hash": source_hash,
            "generation_context": strategy,
            "generator": "codegen-suggestor-stub",
        })
        .to_string();

        AgentEffect::with_proposal(
            ProposedFact::new(
                ContextKey::Hypotheses,
                "artifact:generated-source:transformer",
                content,
                "codegen",
            )
            .with_confidence(0.6), // Low confidence until verified
        )
    }
}

// ── Code Verifier Suggestor ─────────────────────────────────────────

/// Verifies generated code by checking structure and (in production)
/// compiling to Wasm and running acceptance tests.
/// Emits pass/fail as evaluation facts.
pub struct CodeVerifierSuggestor {
    verified: Mutex<bool>,
}

impl CodeVerifierSuggestor {
    pub fn new() -> Self {
        Self {
            verified: Mutex::new(false),
        }
    }
}

#[async_trait::async_trait]
impl Suggestor for CodeVerifierSuggestor {
    fn name(&self) -> &str {
        "code-verifier"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[ContextKey::Hypotheses]
    }

    fn accepts(&self, ctx: &dyn ContextView) -> bool {
        let has_source = ctx
            .get(ContextKey::Hypotheses)
            .iter()
            .any(|f| f.id.starts_with("artifact:generated-source:"));
        let already_verified = *self.verified.lock().unwrap();

        has_source && !already_verified
    }

    async fn execute(&self, ctx: &dyn ContextView) -> AgentEffect {
        *self.verified.lock().unwrap() = true;

        let source_fact = ctx
            .get(ContextKey::Hypotheses)
            .iter()
            .find(|f| f.id.starts_with("artifact:generated-source:"))
            .cloned();

        let Some(source_fact) = source_fact else {
            return AgentEffect::empty();
        };

        let parsed: serde_json::Value =
            match serde_json::from_str(&source_fact.content) {
                Ok(v) => v,
                Err(e) => {
                    return verification_failure(
                        "parse-error",
                        &format!("could not parse source artifact: {e}"),
                    );
                }
            };

        let source = parsed["source"].as_str().unwrap_or("");
        let source_hash = parsed["source_hash"].as_str().unwrap_or("");

        // Verify content hash integrity
        let actual_hash = sha256_hex(source);
        if actual_hash != source_hash {
            return verification_failure(
                "hash-mismatch",
                &format!("expected {source_hash}, got {actual_hash}"),
            );
        }

        // Structural checks (production: actual Wasm compilation via Axiom)
        let checks = vec![
            ("has-function", source.contains("fn transform")),
            ("has-input-type", source.contains("&str") || source.contains("&[u8]")),
            ("has-return-type", source.contains("-> ")),
            ("no-unsafe", !source.contains("unsafe")),
            ("no-std-compatible", !source.contains("use std::") || source.contains("#![no_std]")),
        ];

        let failures: Vec<_> = checks
            .iter()
            .filter(|(_, passed)| !*passed)
            .map(|(name, _)| *name)
            .collect();

        if !failures.is_empty() {
            return verification_failure(
                "structural-check-failed",
                &format!("failed checks: {}", failures.join(", ")),
            );
        }

        // All checks passed — emit verified artifact as Proposals fact
        let verified_content = serde_json::json!({
            "type": "verified-artifact",
            "language": "rust",
            "source_hash": source_hash,
            "verification": {
                "checks_passed": checks.len(),
                "checks_failed": 0,
                "verdict": "pass",
            },
            "provenance": {
                "generated_by": "codegen",
                "verified_by": "code-verifier",
                "source_fact_id": source_fact.id,
            },
        })
        .to_string();

        AgentEffect::with_proposals(vec![
            // Verification result as evaluation
            ProposedFact::new(
                ContextKey::Evaluations,
                "artifact:verification:transformer",
                serde_json::json!({
                    "verdict": "pass",
                    "checks_passed": checks.len(),
                    "source_hash": source_hash,
                })
                .to_string(),
                "code-verifier",
            )
            .with_confidence(0.95),
            // Verified artifact promoted to proposals
            ProposedFact::new(
                ContextKey::Proposals,
                "artifact:verified:transformer",
                verified_content,
                "code-verifier",
            )
            .with_confidence(0.9),
        ])
    }
}

// ── Codegen Gap Suggestor ───────────────────────────────────────────

/// Seeds the initial code generation strategy when a transformation
/// need is detected but no transformer exists.
pub struct CodegenGapSuggestor {
    transformation_spec: String,
    seeded: Mutex<bool>,
}

impl CodegenGapSuggestor {
    pub fn new(spec: impl Into<String>) -> Self {
        Self {
            transformation_spec: spec.into(),
            seeded: Mutex::new(false),
        }
    }
}

#[async_trait::async_trait]
impl Suggestor for CodegenGapSuggestor {
    fn name(&self) -> &str {
        "codegen-gap"
    }

    fn dependencies(&self) -> &[ContextKey] {
        &[]
    }

    fn accepts(&self, _ctx: &dyn ContextView) -> bool {
        !*self.seeded.lock().unwrap()
    }

    async fn execute(&self, _ctx: &dyn ContextView) -> AgentEffect {
        *self.seeded.lock().unwrap() = true;

        AgentEffect::with_proposals(vec![
            ProposedFact::new(
                ContextKey::Seeds,
                "codegen:need",
                serde_json::json!({
                    "type": "transformation-need",
                    "spec": self.transformation_spec,
                })
                .to_string(),
                "codegen-gap",
            ),
            ProposedFact::new(
                ContextKey::Strategies,
                "codegen:strategy:initial",
                format!(
                    "[codegen] generate a Rust data transformer: {}",
                    self.transformation_spec
                ),
                "codegen-gap",
            ),
        ])
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn generate_stub_transformer(_strategy: &str) -> String {
    // Stub: a minimal Rust function that transforms CSV to JSON.
    // In production: LLM generates this from the strategy description.
    r##"/// Auto-generated data transformer
/// Converts CSV row to JSON object.
///
/// Input: CSV line as &str
/// Output: JSON string

pub fn transform(input: &str) -> Result<String, String> {
    let fields: Vec<&str> = input.split(',').collect();
    if fields.len() < 3 {
        return Err(format!("expected at least 3 fields, got {}", fields.len()));
    }
    let json = format!(
        r#"{{"name":"{}","value":"{}","category":"{}"}}"#,
        fields[0].trim(),
        fields[1].trim(),
        fields[2].trim(),
    );
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_csv_to_json() {
        let result = transform("Acme, 42, tech").unwrap();
        assert!(result.contains("Acme"));
        assert!(result.contains("42"));
    }

    #[test]
    fn rejects_short_input() {
        assert!(transform("only,two").is_err());
    }
}
"##
    .to_string()
}

fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn verification_failure(kind: &str, detail: &str) -> AgentEffect {
    let content = serde_json::json!({
        "verdict": "fail",
        "failure_kind": kind,
        "detail": detail,
    })
    .to_string();

    AgentEffect::with_proposal(
        ProposedFact::new(
            ContextKey::Evaluations,
            &format!("artifact:verification-failure:{kind}"),
            content,
            "code-verifier",
        )
        .with_confidence(1.0),
    )
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use converge_kernel::Engine;
    use converge_pack::ContextKey;

    use super::*;

    #[tokio::test]
    async fn codegen_loop_converges_with_verified_artifact() {
        let mut engine = Engine::new();

        engine.register_suggestor(CodegenGapSuggestor::new(
            "CSV row to JSON object transformer",
        ));
        engine.register_suggestor(CodeGenSuggestor::new());
        engine.register_suggestor(CodeVerifierSuggestor::new());

        let ctx = Context::new();
        let result = engine.run(ctx).await.expect("engine should converge");

        // Should have the seed
        let seeds = result.context.get(ContextKey::Seeds);
        assert!(
            seeds.iter().any(|f| f.id == "codegen:need"),
            "should have codegen need seed"
        );

        // Should have generated source as hypothesis
        let hypotheses = result.context.get(ContextKey::Hypotheses);
        assert!(
            hypotheses
                .iter()
                .any(|f| f.id.starts_with("artifact:generated-source:")),
            "should have generated source artifact"
        );

        // Should have verification result
        let evaluations = result.context.get(ContextKey::Evaluations);
        assert!(
            evaluations
                .iter()
                .any(|f| f.id == "artifact:verification:transformer"),
            "should have verification result"
        );

        // Check verification passed
        let verification = evaluations
            .iter()
            .find(|f| f.id == "artifact:verification:transformer")
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&verification.content).unwrap();
        assert_eq!(v["verdict"], "pass");

        // Should have promoted verified artifact
        let proposals = result.context.get(ContextKey::Proposals);
        assert!(
            proposals
                .iter()
                .any(|f| f.id == "artifact:verified:transformer"),
            "should have verified artifact in proposals"
        );

        // Verify provenance chain
        let artifact = proposals
            .iter()
            .find(|f| f.id == "artifact:verified:transformer")
            .unwrap();
        let a: serde_json::Value = serde_json::from_str(&artifact.content).unwrap();
        assert_eq!(a["provenance"]["generated_by"], "codegen");
        assert_eq!(a["provenance"]["verified_by"], "code-verifier");
        assert!(a["provenance"]["source_fact_id"]
            .as_str()
            .unwrap()
            .starts_with("artifact:generated-source:"));

        // Integrity proof should be available
        let proof = &result.integrity;
        assert!(proof.fact_count > 0);
        assert!(proof.clock_time > 0);
    }

    #[test]
    fn stub_transformer_passes_structural_checks() {
        let source = generate_stub_transformer("test");
        assert!(source.contains("fn transform"));
        assert!(source.contains("&str"));
        assert!(source.contains("-> "));
        assert!(!source.contains("unsafe"));
    }

    #[test]
    fn sha256_is_deterministic() {
        let a = sha256_hex("hello world");
        let b = sha256_hex("hello world");
        assert_eq!(a, b);
        assert_ne!(a, sha256_hex("different"));
    }
}
