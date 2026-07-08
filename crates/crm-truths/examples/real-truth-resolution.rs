use crm_truths::{all_truths, converge_binding_for_truth};

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

        if let Some(binding) = converge_binding_for_truth(truth.key) {
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
