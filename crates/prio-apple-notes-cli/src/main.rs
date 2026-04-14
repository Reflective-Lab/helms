use organism_notes::sources::apple_notes::{
    AppleNotesImportProgress, import_apple_notes_with_progress, publish_apple_notes,
    scan_apple_notes,
};
use organism_notes::vault::ObsidianVault;
use serde_json::json;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut probe = false;
    let mut publish = false;
    let mut run_id: Option<String> = None;

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("prio-apple-notes-cli");
        println!();
        println!("Usage:");
        println!("  cargo run -p prio-apple-notes-cli");
        println!("  cargo run -p prio-apple-notes-cli -- --probe");
        println!("  cargo run -p prio-apple-notes-cli -- --publish [--run-id <id>]");
        return Ok(());
    }

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--probe" => {
                probe = true;
                index += 1;
            }
            "--publish" => {
                publish = true;
                index += 1;
            }
            "--run-id" => {
                let Some(value) = args.get(index + 1) else {
                    return Err("--run-id requires a value".into());
                };
                run_id = Some(value.clone());
                index += 2;
            }
            _ => {
                return Err(format!("unsupported arguments: {}", args.join(" ")).into());
            }
        }
    }

    if probe && publish {
        return Err("--probe and --publish cannot be combined".into());
    }

    let vault = ObsidianVault::default_in_home()?;
    vault.ensure_root()?;

    if probe {
        eprintln!("Probing Apple Notes access. macOS may show an Automation prompt for Terminal.");
        let scan = scan_apple_notes()?;
        println!("{}", serde_json::to_string_pretty(&scan)?);
        return Ok(());
    }

    if publish {
        eprintln!("Publishing Apple Notes into {}", vault.root().display());
        let report = publish_apple_notes(&vault, run_id.as_deref())?;
        let published_root = vault.root().join(&report.published_root);
        let report_path = vault.root().join(&report.report_path);
        let output = json!({
            "vault_root": vault.root().display().to_string(),
            "run_id": report.run_id,
            "published_at": report.published_at,
            "source_run_id": report.source_run_id,
            "source_raw_root": report.source_raw_root,
            "source_manifest_path": report.source_manifest_path,
            "published_root": report.published_root,
            "published_path": published_root.display().to_string(),
            "report_path": report.report_path,
            "report_file": report_path.display().to_string(),
            "note_count": report.note_count,
            "attachment_count": report.attachment_count,
            "created_note_count": report.created_note_count,
            "updated_note_count": report.updated_note_count,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    eprintln!("Importing Apple Notes into {}", vault.root().display());
    eprintln!("macOS may show an Automation prompt for Terminal to control Notes.");

    let report = import_apple_notes_with_progress(&vault, |progress| match progress {
        AppleNotesImportProgress::IndexingExistingImports => {
            eprintln!("Indexing existing Apple Notes imports...");
        }
        AppleNotesImportProgress::ExistingImportsIndexed {
            reusable_note_count,
        } => {
            eprintln!(
                "Indexed {} reusable notes from prior Apple Notes imports.",
                reusable_note_count
            );
        }
        AppleNotesImportProgress::ScanningLibrary => {
            eprintln!("Scanning Notes library...");
        }
        AppleNotesImportProgress::LibraryScanned(scan) => {
            eprintln!(
                "Found {} accounts, {} folders, {} notes.",
                scan.account_count, scan.folder_count, scan.note_count
            );
        }
        AppleNotesImportProgress::ExportingFolder {
            completed_folders,
            total_folders,
            account,
            folder,
            note_count,
        } => {
            eprintln!(
                "Exporting folder {}/{}: {}/{} ({} notes)",
                completed_folders + 1,
                total_folders,
                account,
                folder,
                note_count
            );
        }
        AppleNotesImportProgress::ExportingBatch {
            completed_folders,
            total_folders,
            account,
            folder,
            batch_start,
            batch_end,
            folder_note_count,
        } => {
            eprintln!(
                "  Batch {}/{}: {}/{} notes {}-{} of {}",
                completed_folders + 1,
                total_folders,
                account,
                folder,
                batch_start,
                batch_end,
                folder_note_count
            );
        }
        AppleNotesImportProgress::WritingNotes { total } => {
            eprintln!("Writing {} notes into the vault...", total);
        }
        AppleNotesImportProgress::NoteWritten {
            completed,
            total,
            relative_path,
        } => {
            eprintln!("Imported {}/{}: {}", completed, total, relative_path);
        }
    })?;

    let imported_path = vault.root().join(&report.imported_root);
    let raw_path = vault.root().join(&report.raw_root);
    let note_path = vault.root().join(&report.note_root);
    let manifest_path = vault.root().join(&report.manifest_path);
    let output = json!({
        "vault_root": vault.root().display().to_string(),
        "run_id": report.run_id,
        "imported_root": report.imported_root,
        "imported_path": imported_path.display().to_string(),
        "raw_root": report.raw_root,
        "raw_path": raw_path.display().to_string(),
        "note_root": report.note_root,
        "note_path": note_path.display().to_string(),
        "manifest_path": manifest_path.display().to_string(),
        "note_count": report.note_count,
        "attachment_count": report.attachment_count,
        "reused_note_count": report.reused_note_count,
        "reused_attachment_count": report.reused_attachment_count,
        "locked_note_count": report.locked_note_count,
        "timed_out_note_count": report.timed_out_note_count,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
