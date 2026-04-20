use truth_catalog::{all_truths, converge_binding_for_truth, organism_binding_for_truth};

fn main() {
    let requested_keys = std::env::args().skip(1).collect::<Vec<_>>();
    let truths = all_truths();
    let selected = if requested_keys.is_empty() {
        truths
    } else {
        truths
            .into_iter()
            .filter(|truth| requested_keys.iter().any(|key| key == truth.key))
            .collect()
    };

    if selected.is_empty() {
        eprintln!("No matching truths.");
        std::process::exit(1);
    }

    for truth in selected {
        println!("=== {} ({}) ===", truth.display_name, truth.key);
        println!("{}", truth.summary);
        println!("feature: {}", truth.feature_path);

        if let Some(binding) = organism_binding_for_truth(truth.key) {
            let truth_catalog::TruthOrganismBinding {
                blueprint,
                binding,
                readiness,
                ..
            } = binding;
            println!("runtime: organism");
            if let Some(blueprint) = blueprint {
                println!("blueprint: {blueprint}");
            }

            if binding.packs.is_empty() {
                println!("packs: none");
            } else {
                println!("packs:");
                for pack in binding.packs {
                    println!(
                        "  - {} [{}] {:.0}% {}",
                        pack.pack_name,
                        format!("{:?}", pack.source).to_ascii_lowercase(),
                        pack.confidence * 100.0,
                        pack.reason
                    );
                }
            }

            if binding.capabilities.is_empty() {
                println!("capabilities: none");
            } else {
                println!("capabilities:");
                for capability in binding.capabilities {
                    println!(
                        "  - {} [{}] {:.0}% {}",
                        capability.capability,
                        format!("{:?}", capability.source).to_ascii_lowercase(),
                        capability.confidence * 100.0,
                        capability.reason
                    );
                }
            }

            if binding.invariants.is_empty() {
                println!("invariants: none");
            } else {
                println!("invariants: {}", binding.invariants.join(", "));
            }

            println!(
                "readiness: {}",
                if readiness.ready {
                    "ready"
                } else {
                    "not ready"
                }
            );
            for gap in readiness.gaps {
                println!(
                    "  gap: {} [{}] {}",
                    gap.resource,
                    format!("{:?}", gap.severity).to_ascii_lowercase(),
                    gap.reason
                );
            }
        } else if let Some(binding) = converge_binding_for_truth(truth.key) {
            println!("runtime: {}", binding.runtime);
            println!("packs: {}", binding.pack_ids.join(", "));
            println!("request: {}", binding.intent.request);
            let required = binding.required_success_criteria();
            println!(
                "required success criteria: {}",
                if required.is_empty() {
                    "none".to_string()
                } else {
                    required.join(" | ")
                }
            );
            let constraints = binding.hard_constraints();
            println!(
                "hard constraints: {}",
                if constraints.is_empty() {
                    "none".to_string()
                } else {
                    constraints.join(" | ")
                }
            );
        } else {
            println!("runtime: none");
        }

        println!();
    }
}
