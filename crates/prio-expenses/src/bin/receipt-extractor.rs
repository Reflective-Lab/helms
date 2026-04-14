use std::process::ExitCode;

use organism_intelligence::ocr::{OllamaReceiptConfig, TesseractCliConfig};
use prio_expenses::receipt_extractor::{
    ExtractorEngine, ReceiptExtractorError, benchmark_output, discover_receipt_fixture_root,
    find_sample, load_receipt_samples,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), ReceiptExtractorError> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return Ok(());
    };

    let root = discover_receipt_fixture_root()?;
    let samples = load_receipt_samples(&root)?;

    match command {
        "list" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&samples).unwrap_or_default()
            );
        }
        "extract" => {
            let engine = parse_engine(args.get(1).map(String::as_str))?;
            let sample_id = args.get(2).ok_or_else(|| {
                ReceiptExtractorError::SampleNotFound(
                    "missing sample id for extract command".to_string(),
                )
            })?;
            let sample = find_sample(&samples, sample_id)?;
            let output = engine.extract(sample)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&output).unwrap_or_default()
            );
        }
        "benchmark" => {
            let engine = parse_engine(args.get(1).map(String::as_str))?;
            let selected = if let Some(sample_id) = args.get(2) {
                vec![find_sample(&samples, sample_id)?.clone()]
            } else {
                samples.clone()
            };
            let mut reports = Vec::new();
            for sample in &selected {
                let output = engine.extract(sample)?;
                reports.push(benchmark_output(&sample.fixture.expected, &output));
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&reports).unwrap_or_default()
            );
        }
        _ => print_usage(),
    }

    Ok(())
}

fn parse_engine(name: Option<&str>) -> Result<ExtractorEngine, ReceiptExtractorError> {
    match name.unwrap_or("canonical-name") {
        "reference" => Ok(ExtractorEngine::Reference),
        "canonical-name" => Ok(ExtractorEngine::CanonicalName),
        "tesseract-cli" => Ok(ExtractorEngine::TesseractCli(TesseractCliConfig::default())),
        "ollama-glm-ocr" | "ollama" => Ok(ExtractorEngine::Ollama(OllamaReceiptConfig::default())),
        value => Err(ReceiptExtractorError::EngineFailed {
            engine: "cli",
            path: value.to_string(),
            message:
                "unknown engine; use reference, canonical-name, tesseract-cli, or ollama-glm-ocr"
                    .to_string(),
        }),
    }
}

fn print_usage() {
    eprintln!(
        "usage:\n  cargo run -p prio-expenses --bin receipt-extractor -- list\n  \
         cargo run -p prio-expenses --bin receipt-extractor -- extract <engine> <sample-id>\n  \
         cargo run -p prio-expenses --bin receipt-extractor -- benchmark <engine> [sample-id]"
    );
}
