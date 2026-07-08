/// Proto compilation for the CRM Helm scenario.
///
/// Strategy: proto files are copied into scenarios/crm-helm/proto/ from
/// helms/proto/ rather than referenced by a cross-repo path.  This decouples
/// the atelier-showcase workspace from the helms filesystem layout and avoids
/// fragile relative paths that cross workspace roots.  The copy-and-own
/// approach is intentional — atelier-showcase is a showcase/demo layer, not
/// a production dependency of helms.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(
            &[
                "proto/prio/common/v1/common.proto",
                "proto/prio/parties/v1/parties.proto",
                "proto/prio/opportunities/v1/opportunities.proto",
                "proto/prio/conversations/v1/conversations.proto",
                "proto/prio/documents/v1/documents.proto",
                "proto/prio/workflow/v1/workflow.proto",
                "proto/prio/facts/v1/facts.proto",
                "proto/prio/metadata/v1/metadata.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
