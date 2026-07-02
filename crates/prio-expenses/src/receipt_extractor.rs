use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use chrono::NaiveDate;
use organism_intelligence::ocr::{
    OcrError as OcrBridgeError, OcrOutputFormat, OcrProvider, OcrResult, OllamaReceiptConfig,
    OllamaReceiptOcrProvider, TesseractCliConfig, TesseractCliOcrProvider, ocr_request_for_path,
};
use organism_intelligence::pdf::PdfIngester;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptReference {
    pub schema_version: u32,
    pub sample_id: String,
    pub document_file: String,
    pub original_file_name: String,
    pub document_type: String,
    pub capture_type: String,
    pub expense_candidate: bool,
    pub related_document_file: String,
    pub reference_status: String,
    pub expected: ReceiptExpectedFields,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptExpectedFields {
    #[serde(default)]
    pub merchant: String,
    #[serde(default)]
    pub issue_date: String,
    #[serde(default)]
    pub service_date: String,
    #[serde(default)]
    pub service_period_start: String,
    #[serde(default)]
    pub service_period_end: String,
    #[serde(default)]
    pub due_date: String,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub total: String,
    #[serde(default)]
    pub subtotal: String,
    #[serde(default)]
    pub tax: String,
    #[serde(default)]
    pub tax_rate: String,
    #[serde(default)]
    pub invoice_number: String,
    #[serde(default)]
    pub receipt_number: String,
    #[serde(default)]
    pub order_id: String,
    #[serde(default)]
    pub account_reference: String,
    #[serde(default)]
    pub country: String,
}

impl ReceiptExpectedFields {
    pub fn to_field_map(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        for (key, value) in self.non_empty_fields() {
            fields.insert(key.to_string(), value.to_string());
        }
        fields
    }

    pub fn non_empty_fields(&self) -> Vec<(&'static str, &str)> {
        let mut values = Vec::new();
        for (key, value) in [
            ("merchant", self.merchant.as_str()),
            ("issue_date", self.issue_date.as_str()),
            ("service_date", self.service_date.as_str()),
            ("service_period_start", self.service_period_start.as_str()),
            ("service_period_end", self.service_period_end.as_str()),
            ("due_date", self.due_date.as_str()),
            ("currency", self.currency.as_str()),
            ("total", self.total.as_str()),
            ("subtotal", self.subtotal.as_str()),
            ("tax", self.tax.as_str()),
            ("tax_rate", self.tax_rate.as_str()),
            ("invoice_number", self.invoice_number.as_str()),
            ("receipt_number", self.receipt_number.as_str()),
            ("order_id", self.order_id.as_str()),
            ("account_reference", self.account_reference.as_str()),
            ("country", self.country.as_str()),
        ] {
            if !value.trim().is_empty() {
                values.push((key, value.trim()));
            }
        }
        values
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptSample {
    pub fixture: ReceiptReference,
    pub document_path: PathBuf,
    pub reference_path: PathBuf,
}

impl ReceiptSample {
    pub fn sample_id(&self) -> &str {
        &self.fixture.sample_id
    }

    pub fn document_extension(&self) -> Option<&str> {
        self.document_path.extension().and_then(OsStr::to_str)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractionOutput {
    pub engine: String,
    pub implementation: String,
    pub sample_id: String,
    pub fields: BTreeMap<String, String>,
    #[serde(default)]
    pub raw_text: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtractionBenchmark {
    pub engine: String,
    pub sample_id: String,
    pub matched_fields: usize,
    pub compared_fields: usize,
    pub missing_fields: Vec<FieldComparison>,
    pub mismatched_fields: Vec<FieldComparison>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldComparison {
    pub field: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Clone)]
pub enum ExtractorEngine {
    Reference,
    CanonicalName,
    TesseractCli(TesseractCliConfig),
    Ollama(OllamaReceiptConfig),
}

impl ExtractorEngine {
    pub fn engine_name(&self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::CanonicalName => "canonical-name",
            Self::TesseractCli(_) => "tesseract-cli",
            Self::Ollama(_) => "ollama-glm-ocr",
        }
    }

    pub fn extract(
        &self,
        sample: &ReceiptSample,
    ) -> Result<ExtractionOutput, ReceiptExtractorError> {
        match self {
            Self::Reference => Ok(reference_extract(sample)),
            Self::CanonicalName => Ok(canonical_name_extract(sample)),
            Self::TesseractCli(config) => tesseract_extract(sample, config),
            Self::Ollama(config) => ollama_extract(sample, config),
        }
    }
}

#[derive(Debug, Error)]
pub enum ReceiptExtractorError {
    #[error("I/O failed for {path}: {message}")]
    Io { path: String, message: String },
    #[error("reference file is invalid at {path}: {message}")]
    InvalidReference { path: String, message: String },
    #[error("fixture root not found: {0}")]
    FixtureRootNotFound(String),
    #[error("sample not found: {0}")]
    SampleNotFound(String),
    #[error("unsupported document type for {engine}: {path}")]
    UnsupportedDocument { engine: &'static str, path: String },
    #[error("{engine} dependency is unavailable: {message}")]
    DependencyUnavailable {
        engine: &'static str,
        message: String,
    },
    #[error("{engine} failed for {path}: {message}")]
    EngineFailed {
        engine: &'static str,
        path: String,
        message: String,
    },
    #[error("structured response could not be parsed: {0}")]
    InvalidStructuredResponse(String),
}

pub fn discover_receipt_fixture_root() -> Result<PathBuf, ReceiptExtractorError> {
    let mut current = std::env::current_dir().map_err(|error| ReceiptExtractorError::Io {
        path: ".".to_string(),
        message: error.to_string(),
    })?;

    loop {
        let candidate = current.join("data/receipts");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        if !current.pop() {
            break;
        }
    }

    Err(ReceiptExtractorError::FixtureRootNotFound(
        "could not find data/receipts by walking up from the current directory".to_string(),
    ))
}

pub fn load_receipt_samples(
    root: impl AsRef<Path>,
) -> Result<Vec<ReceiptSample>, ReceiptExtractorError> {
    let root = root.as_ref();
    let mut samples = Vec::new();

    for entry in fs::read_dir(root).map_err(|error| ReceiptExtractorError::Io {
        path: root.display().to_string(),
        message: error.to_string(),
    })? {
        let entry = entry.map_err(|error| ReceiptExtractorError::Io {
            path: root.display().to_string(),
            message: error.to_string(),
        })?;
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        if !file_name.ends_with(".reference.yaml") {
            continue;
        }

        let content = fs::read_to_string(&path).map_err(|error| ReceiptExtractorError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        let fixture: ReceiptReference = serde_yaml::from_str(&content).map_err(|error| {
            ReceiptExtractorError::InvalidReference {
                path: path.display().to_string(),
                message: error.to_string(),
            }
        })?;
        let document_path = root.join(&fixture.document_file);
        if !document_path.is_file() {
            return Err(ReceiptExtractorError::InvalidReference {
                path: path.display().to_string(),
                message: format!(
                    "document_file {:?} does not exist under {}",
                    fixture.document_file,
                    root.display()
                ),
            });
        }

        samples.push(ReceiptSample {
            fixture,
            document_path,
            reference_path: path,
        });
    }

    samples.sort_by(|left, right| left.fixture.sample_id.cmp(&right.fixture.sample_id));
    Ok(samples)
}

pub fn find_sample<'a>(
    samples: &'a [ReceiptSample],
    sample_id: &str,
) -> Result<&'a ReceiptSample, ReceiptExtractorError> {
    samples
        .iter()
        .find(|sample| sample.sample_id() == sample_id)
        .ok_or_else(|| ReceiptExtractorError::SampleNotFound(sample_id.to_string()))
}

pub fn benchmark_output(
    reference: &ReceiptExpectedFields,
    output: &ExtractionOutput,
) -> ExtractionBenchmark {
    let mut missing_fields = Vec::new();
    let mut mismatched_fields = Vec::new();
    let mut matched_fields = 0;

    for (field, expected) in reference.non_empty_fields() {
        let actual = output.fields.get(field).cloned().unwrap_or_default();
        if actual.trim().is_empty() {
            missing_fields.push(FieldComparison {
                field: field.to_string(),
                expected: expected.to_string(),
                actual,
            });
            continue;
        }

        if comparable_value(field, expected) == comparable_value(field, &actual) {
            matched_fields += 1;
        } else {
            mismatched_fields.push(FieldComparison {
                field: field.to_string(),
                expected: expected.to_string(),
                actual,
            });
        }
    }

    ExtractionBenchmark {
        engine: output.engine.clone(),
        sample_id: output.sample_id.clone(),
        matched_fields,
        compared_fields: reference.non_empty_fields().len(),
        missing_fields,
        mismatched_fields,
    }
}

fn reference_extract(sample: &ReceiptSample) -> ExtractionOutput {
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "document_type".to_string(),
        sample.fixture.document_type.clone(),
    );
    metadata.insert(
        "capture_type".to_string(),
        sample.fixture.capture_type.clone(),
    );
    metadata.insert(
        "expense_candidate".to_string(),
        sample.fixture.expense_candidate.to_string(),
    );

    ExtractionOutput {
        engine: "reference".to_string(),
        implementation: "yaml-sidecar".to_string(),
        sample_id: sample.sample_id().to_string(),
        fields: sample.fixture.expected.to_field_map(),
        raw_text: None,
        warnings: vec![],
        metadata,
    }
}

fn canonical_name_extract(sample: &ReceiptSample) -> ExtractionOutput {
    let stem = sample
        .document_path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or_default();
    let tokens = stem
        .split('-')
        .map(|token| token.trim())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    let mut fields = BTreeMap::new();
    let document_kind = tokens.first().copied().unwrap_or_default();
    if !document_kind.is_empty() {
        fields.insert("document_type".to_string(), document_kind.to_string());
    }

    if let Some(issue_date) = parse_stem_date(&tokens) {
        fields.insert("issue_date".to_string(), issue_date);
    }

    if let Some(merchant) = parse_stem_merchant(&tokens) {
        fields.insert("merchant".to_string(), merchant);
    }

    ExtractionOutput {
        engine: "canonical-name".to_string(),
        implementation: "path-tokenizer".to_string(),
        sample_id: sample.sample_id().to_string(),
        fields,
        raw_text: None,
        warnings: vec!["canonical-name is a routing baseline, not a full OCR engine".to_string()],
        metadata: BTreeMap::from([
            (
                "document_file".to_string(),
                sample.fixture.document_file.clone(),
            ),
            (
                "capture_type".to_string(),
                sample.fixture.capture_type.clone(),
            ),
        ]),
    }
}

fn tesseract_extract(
    sample: &ReceiptSample,
    config: &TesseractCliConfig,
) -> Result<ExtractionOutput, ReceiptExtractorError> {
    let provider = TesseractCliOcrProvider::with_config(config.clone());
    let mut fields = canonical_name_extract(sample).fields;
    let mut metadata = BTreeMap::new();
    let text = if let Some(text) = preferred_direct_text(sample)? {
        metadata.insert(
            "source_kind".to_string(),
            direct_text_source_kind(sample).to_string(),
        );
        metadata.insert("ocr_skipped".to_string(), "true".to_string());
        text
    } else {
        let request = ocr_request_for_path(
            &sample.document_path,
            OcrOutputFormat::Text,
            config.languages.clone(),
        )
        .map_err(|error| map_ocr_error(sample, "tesseract-cli", error))?;
        let result = provider
            .extract(&request)
            .map_err(|error| map_ocr_error(sample, "tesseract-cli", error))?;
        extend_metadata_with_ocr_result(&mut metadata, &result);
        result.text
    };
    merge_text_fields(&mut fields, &text);

    Ok(ExtractionOutput {
        engine: "tesseract-cli".to_string(),
        implementation: format!("organism-intelligence:{}", provider.model()),
        sample_id: sample.sample_id().to_string(),
        fields,
        raw_text: Some(text),
        warnings: vec![],
        metadata,
    })
}

fn ollama_extract(
    sample: &ReceiptSample,
    config: &OllamaReceiptConfig,
) -> Result<ExtractionOutput, ReceiptExtractorError> {
    let provider = OllamaReceiptOcrProvider::with_config(config.clone());
    // Default HTTP client for the direct-text path. The provider used in the
    // OCR branch owns its own client via the DI pattern (organism's
    // RP-HERMETIC-UNIT migration); this one covers the text-only branch.
    // Tests that exercise `send_ollama_request` / `query_ollama_with_text`
    // construct their own stub client and call the helpers directly.
    #[allow(clippy::disallowed_methods)]
    let client = reqwest::blocking::Client::builder()
        .build()
        .map_err(|error| ReceiptExtractorError::EngineFailed {
            engine: "ollama-glm-ocr",
            path: config.base_url.clone(),
            message: error.to_string(),
        })?;
    let mut fields = canonical_name_extract(sample).fields;
    let prompt = json_prompt();
    let mut metadata = BTreeMap::new();
    metadata.insert("base_url".to_string(), config.base_url.clone());
    metadata.insert("model".to_string(), config.model.clone());

    let response_text = if let Some(text) = preferred_direct_text(sample)? {
        metadata.insert(
            "source_kind".to_string(),
            direct_text_source_kind(sample).to_string(),
        );
        metadata.insert("ocr_skipped".to_string(), "true".to_string());
        query_ollama_with_text(&client, &text, config, prompt)?
    } else {
        let request = ocr_request_for_path(&sample.document_path, OcrOutputFormat::Json, vec![])
            .map_err(|error| map_ocr_error(sample, "ollama-glm-ocr", error))?;
        let result = provider
            .extract(&request)
            .map_err(|error| map_ocr_error(sample, "ollama-glm-ocr", error))?;
        extend_metadata_with_ocr_result(&mut metadata, &result);
        result.text
    };

    let mut warnings = Vec::new();
    if let Some(parsed) = parse_json_object_from_response(&response_text) {
        for (key, value) in parsed {
            if let Some(value) = value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                fields.insert(key, value.to_string());
            }
        }
    } else {
        warnings.push("Ollama response did not contain a parseable JSON object".to_string());
        merge_text_fields(&mut fields, &response_text);
    }

    Ok(ExtractionOutput {
        engine: "ollama-glm-ocr".to_string(),
        implementation: format!("organism-intelligence:{}", provider.model()),
        sample_id: sample.sample_id().to_string(),
        fields,
        raw_text: Some(response_text),
        warnings,
        metadata,
    })
}

fn comparable_value(field: &str, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }

    match field {
        "merchant" => normalize_text_for_identity(value),
        "issue_date"
        | "service_date"
        | "service_period_start"
        | "service_period_end"
        | "due_date" => normalize_date_string(value).unwrap_or_else(|| normalize_whitespace(value)),
        "total" | "subtotal" | "tax" => {
            normalize_amount_string(value).unwrap_or_else(|| normalize_whitespace(value))
        }
        "tax_rate" => {
            normalize_percentage_string(value).unwrap_or_else(|| normalize_whitespace(value))
        }
        _ => normalize_whitespace(value).to_lowercase(),
    }
}

fn normalize_text_for_identity(value: &str) -> String {
    normalize_whitespace(
        &value
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character.is_whitespace() {
                    character
                } else {
                    ' '
                }
            })
            .collect::<String>(),
    )
    .to_lowercase()
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_percentage_string(value: &str) -> Option<String> {
    let digits = value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        Some(format!("{digits}%"))
    }
}

fn normalize_amount_string(value: &str) -> Option<String> {
    let mut raw = value
        .replace('\u{00a0}', " ")
        .replace("SEK", "")
        .replace("EUR", "")
        .replace("USD", "")
        .replace(['€', '$'], "")
        .replace("kr", "");
    raw = raw.trim().to_string();
    if raw.is_empty() {
        return None;
    }

    let has_comma = raw.contains(',');
    let has_dot = raw.contains('.');
    let normalized = match (has_comma, has_dot) {
        (true, true) => raw.replace('.', "").replace(',', "."),
        (true, false) => raw.replace(',', "."),
        _ => raw,
    };

    let filtered = normalized
        .chars()
        .filter(|character| character.is_ascii_digit() || *character == '.')
        .collect::<String>();
    if filtered.is_empty() {
        None
    } else {
        Some(filtered)
    }
}

fn normalize_date_string(value: &str) -> Option<String> {
    let candidates = [
        "%Y-%m-%d",
        "%Y/%m/%d",
        "%d/%m/%Y",
        "%d.%m.%Y",
        "%d %B %Y",
        "%d %b %Y",
        "%B %d %Y",
        "%b %d %Y",
        "%B %d, %Y",
        "%b %d, %Y",
    ];

    for format in candidates {
        if let Ok(date) = NaiveDate::parse_from_str(value, format) {
            return Some(date.format("%Y-%m-%d").to_string());
        }
    }
    None
}

fn parse_stem_date(tokens: &[&str]) -> Option<String> {
    if tokens.len() < 4 {
        return None;
    }
    let year = tokens[tokens.len() - 3];
    let month = tokens[tokens.len() - 2];
    let day = tokens[tokens.len() - 1];
    let candidate = format!("{year}-{month}-{day}");
    normalize_date_string(&candidate)
}

fn parse_stem_merchant(tokens: &[&str]) -> Option<String> {
    if tokens.len() <= 1 {
        return None;
    }

    let qualifiers = [
        "invoice",
        "receipt",
        "statement",
        "email",
        "photo",
        "scan",
        "not",
        "an",
        "max",
        "plan",
        "march",
        "april",
        "january",
        "february",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];

    let parts = tokens[1..]
        .iter()
        .filter(|token| {
            !token.chars().all(|character| character.is_ascii_digit())
                && !token.chars().any(|character| character.is_ascii_digit())
                && !qualifiers.contains(token)
        })
        .take_while(|token| {
            !matches!(
                **token,
                "2023" | "2024" | "2025" | "2026" | "2027" | "2028" | "2029"
            )
        })
        .map(|token| title_case(token))
        .collect::<Vec<_>>();

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn title_case(token: &str) -> String {
    let mut characters = token.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    let mut value = String::new();
    value.extend(first.to_uppercase());
    value.push_str(characters.as_str());
    value
}

fn is_text_like(extension: Option<&str>) -> bool {
    matches!(
        extension,
        Some("rtf" | "txt" | "md" | "json" | "yaml" | "yml")
    )
}

fn is_digital_pdf(sample: &ReceiptSample) -> bool {
    sample.document_extension() == Some("pdf") && sample.fixture.capture_type == "digital-pdf"
}

fn direct_text_source_kind(sample: &ReceiptSample) -> &'static str {
    if is_digital_pdf(sample) {
        "digital-pdf"
    } else {
        "text-like"
    }
}

fn preferred_direct_text(sample: &ReceiptSample) -> Result<Option<String>, ReceiptExtractorError> {
    if is_text_like(sample.document_extension()) {
        return Ok(Some(extract_text_document(sample)?));
    }

    if is_digital_pdf(sample) {
        let text = extract_text_document(sample)?;
        if has_meaningful_direct_text(&text) {
            return Ok(Some(text));
        }
    }

    Ok(None)
}

fn has_meaningful_direct_text(text: &str) -> bool {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .count()
        >= 40
}

fn extract_text_document(sample: &ReceiptSample) -> Result<String, ReceiptExtractorError> {
    match sample.document_extension() {
        Some("rtf") => {
            let path = sample.document_path.to_string_lossy().to_string();
            run_command_text(
                "textutil",
                &["-convert", "txt", "-stdout", path.as_str()],
                sample,
                "native-text",
            )
        }
        Some("pdf") => PdfIngester::new()
            .ingest_file(&sample.document_path)
            .map(|document| document.full_text())
            .map_err(|error| ReceiptExtractorError::EngineFailed {
                engine: "pdf",
                path: sample.document_path.display().to_string(),
                message: error.to_string(),
            }),
        _ => fs::read_to_string(&sample.document_path).map_err(|error| ReceiptExtractorError::Io {
            path: sample.document_path.display().to_string(),
            message: error.to_string(),
        }),
    }
}

fn map_ocr_error(
    sample: &ReceiptSample,
    engine: &'static str,
    error: OcrBridgeError,
) -> ReceiptExtractorError {
    match error {
        OcrBridgeError::InvalidInput(_) => ReceiptExtractorError::UnsupportedDocument {
            engine,
            path: sample.document_path.display().to_string(),
        },
        OcrBridgeError::Network(message)
        | OcrBridgeError::Auth(message)
        | OcrBridgeError::RateLimit(message)
        | OcrBridgeError::Parse(message)
        | OcrBridgeError::Api(message) => ReceiptExtractorError::EngineFailed {
            engine,
            path: sample.document_path.display().to_string(),
            message,
        },
    }
}

fn extend_metadata_with_ocr_result(metadata: &mut BTreeMap<String, String>, result: &OcrResult) {
    metadata.insert(
        "ocr_provider".to_string(),
        result.provenance.provider.clone(),
    );
    metadata.insert("ocr_version".to_string(), result.provenance.version.clone());
    metadata.insert("ocr_pages".to_string(), result.pages.to_string());
    metadata.insert(
        "ocr_languages".to_string(),
        result.provenance.languages.join(","),
    );
    if let Some(processing_time_ms) = result.processing_time_ms {
        metadata.insert(
            "ocr_processing_time_ms".to_string(),
            processing_time_ms.to_string(),
        );
    }
    if let Some(confidence) = &result.confidence {
        metadata.insert(
            "ocr_confidence_mean".to_string(),
            format!("{:.4}", confidence.mean),
        );
        metadata.insert(
            "ocr_low_confidence_words".to_string(),
            confidence.low_confidence_words.to_string(),
        );
    }
    if let Some(dpi) = result.provenance.preprocessing.dpi {
        metadata.insert("ocr_dpi".to_string(), dpi.to_string());
    }
    if let Some(psm) = result.provenance.preprocessing.psm {
        metadata.insert("ocr_psm".to_string(), psm.to_string());
    }
    if let Some(oem) = result.provenance.preprocessing.oem {
        metadata.insert("ocr_oem".to_string(), oem.to_string());
    }
    for (key, value) in &result.provenance.metadata {
        metadata.insert(format!("ocr_{key}"), value.clone());
    }
}

fn query_ollama_with_text(
    client: &reqwest::blocking::Client,
    text: &str,
    config: &OllamaReceiptConfig,
    prompt: &str,
) -> Result<String, ReceiptExtractorError> {
    let request = serde_json::json!({
        "model": config.model,
        "prompt": format!("{prompt}\n\nDocument text:\n{text}"),
        "stream": false,
        "format": "json",
    });
    send_ollama_request(client, config, request)
}

fn send_ollama_request(
    client: &reqwest::blocking::Client,
    config: &OllamaReceiptConfig,
    request: Value,
) -> Result<String, ReceiptExtractorError> {
    let response = client
        .post(format!(
            "{}/api/generate",
            config.base_url.trim_end_matches('/')
        ))
        .json(&request)
        .send()
        .map_err(|error| ReceiptExtractorError::EngineFailed {
            engine: "ollama-glm-ocr",
            path: config.base_url.clone(),
            message: error.to_string(),
        })?;
    let status = response.status();
    let payload: Value = response
        .json()
        .map_err(|error| ReceiptExtractorError::EngineFailed {
            engine: "ollama-glm-ocr",
            path: config.base_url.clone(),
            message: error.to_string(),
        })?;
    if !status.is_success() {
        return Err(ReceiptExtractorError::EngineFailed {
            engine: "ollama-glm-ocr",
            path: config.base_url.clone(),
            message: payload.to_string(),
        });
    }
    Ok(payload
        .get("response")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string())
}

fn parse_json_object_from_response(response: &str) -> Option<BTreeMap<String, Value>> {
    let trimmed = response.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(value) = serde_json::from_str::<BTreeMap<String, Value>>(trimmed) {
        return Some(value);
    }

    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&trimmed[start..=end]).ok()
}

fn json_prompt() -> &'static str {
    "Extract expense document fields and return JSON only. Use empty strings for unknown values. \
Return exactly these keys: merchant, issue_date, service_date, service_period_start, \
service_period_end, due_date, currency, total, subtotal, tax, tax_rate, invoice_number, \
receipt_number, order_id, account_reference, country. Normalize dates to YYYY-MM-DD. \
Normalize amounts to decimal strings using a dot."
}

fn merge_text_fields(fields: &mut BTreeMap<String, String>, text: &str) {
    let normalized = text.replace('\u{00a0}', " ");
    let lines = normalized
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return;
    }

    if !fields.contains_key("merchant")
        && let Some(merchant) = extract_merchant(&lines)
    {
        fields.insert("merchant".to_string(), merchant);
    }

    for (field, value) in [
        (
            "issue_date",
            find_labeled_date(&lines, &["date of issue", "invoice date", "date"]),
        ),
        ("service_date", find_labeled_date(&lines, &["service date"])),
        (
            "service_period_start",
            find_service_period(&normalized).map(|(start, _)| start),
        ),
        (
            "service_period_end",
            find_service_period(&normalized).map(|(_, end)| end),
        ),
        (
            "due_date",
            find_labeled_date(&lines, &["date due", "due date"]),
        ),
        ("currency", find_currency(&normalized)),
        ("total", find_total_amount(&lines)),
        (
            "subtotal",
            find_amount_by_keywords(
                &lines,
                &["subtotal", "net amount", "montant total hors tva"],
            ),
        ),
        (
            "tax",
            find_amount_by_keywords(&lines, &["vat charged", "vat", "tax", "tva"]),
        ),
        (
            "tax_rate",
            find_percentage_by_keywords(&lines, &["vat", "tax", "tva"]),
        ),
        (
            "invoice_number",
            find_label_value(
                &lines,
                &[
                    "invoice number",
                    "document",
                    "invoice no",
                    "faktura",
                    "facture",
                ],
            ),
        ),
        (
            "receipt_number",
            find_label_value(&lines, &["receipt number"]),
        ),
        ("order_id", find_label_value(&lines, &["order id"])),
        (
            "account_reference",
            find_label_value(&lines, &["customer number", "apple account", "account"]),
        ),
        ("country", find_country(&lines)),
    ] {
        if let Some(value) = value {
            fields.entry(field.to_string()).or_insert(value);
        }
    }
}

fn find_total_amount(lines: &[&str]) -> Option<String> {
    find_amount_by_keywords(
        lines,
        &[
            "amount due",
            "amount to pay",
            "total ttc",
            "total net",
            "total",
        ],
    )
    .or_else(|| {
        lines.iter().rev().find_map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.contains(" due ") || lower.ends_with(" due") {
                extract_last_amount(line)
            } else {
                None
            }
        })
    })
}

fn extract_merchant(lines: &[&str]) -> Option<String> {
    let noise = [
        "invoice",
        "receipt",
        "billing and payment",
        "subtotal",
        "amount due",
        "date",
        "page",
    ];
    for line in lines.iter().take(12) {
        let lower = line.to_ascii_lowercase();
        if noise
            .iter()
            .any(|noise| lower == *noise || lower.starts_with(&format!("{noise}:")))
        {
            continue;
        }
        if lower.contains("invoice number") || lower.contains("date of issue") {
            continue;
        }
        if lower.contains("inc.")
            || lower.contains("ab")
            || lower.contains("pbc")
            || lower.contains("primagaz")
            || lower.contains("darty")
            || lower.contains("apple")
            || lower.contains("aspia")
        {
            return Some(normalize_whitespace(line));
        }
    }
    None
}

fn find_labeled_date(lines: &[&str], labels: &[&str]) -> Option<String> {
    for line in lines {
        let lower = line.to_ascii_lowercase();
        for label in labels {
            if !lower.contains(label) {
                continue;
            }
            if let Some(date) = extract_first_date(line) {
                return Some(date);
            }
        }
    }
    None
}

fn find_service_period(text: &str) -> Option<(String, String)> {
    let regex = service_period_regex();
    let captures = regex.captures(text)?;
    let start = normalize_date_string(captures.get(1)?.as_str())?;
    let end = normalize_date_string(captures.get(2)?.as_str())?;
    Some((start, end))
}

fn find_currency(text: &str) -> Option<String> {
    let upper = text.to_ascii_uppercase();
    if upper.contains(" EUR") || text.contains('€') {
        return Some("EUR".to_string());
    }
    if upper.contains(" USD") || text.contains('$') {
        return Some("USD".to_string());
    }
    if upper.contains(" SEK") || upper.contains(" KR") || text.contains("kr") {
        return Some("SEK".to_string());
    }
    None
}

fn find_amount_by_keywords(lines: &[&str], keywords: &[&str]) -> Option<String> {
    for line in lines.iter().rev() {
        let lower = line.to_ascii_lowercase();
        if !keywords.iter().any(|keyword| lower.contains(keyword)) {
            continue;
        }
        if let Some(amount) = extract_last_amount(line) {
            return Some(amount);
        }
    }
    None
}

fn find_percentage_by_keywords(lines: &[&str], keywords: &[&str]) -> Option<String> {
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if !keywords.iter().any(|keyword| lower.contains(keyword)) {
            continue;
        }
        if let Some(rate) = extract_percentage(line) {
            return Some(rate);
        }
    }
    None
}

fn find_label_value(lines: &[&str], labels: &[&str]) -> Option<String> {
    for line in lines {
        let lower = line.to_ascii_lowercase();
        for label in labels {
            if !lower.contains(label) {
                continue;
            }
            let value = line
                .split_once(':')
                .map(|(_, value)| value.trim())
                .filter(|value| !value.is_empty())
                .map(normalize_whitespace)
                .or_else(|| extract_code_after_label(line, label));
            if value.is_some() {
                return value;
            }
        }
    }
    None
}

fn find_country(lines: &[&str]) -> Option<String> {
    for line in lines.iter().rev() {
        let lower = line.to_ascii_lowercase();
        for country in ["sweden", "france", "united states"] {
            if lower.contains(country) {
                return Some(title_case_words(country));
            }
        }
    }
    None
}

fn title_case_words(value: &str) -> String {
    value
        .split_whitespace()
        .map(title_case)
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_first_date(text: &str) -> Option<String> {
    for candidate in date_regex()
        .captures_iter(text)
        .filter_map(|captures| captures.get(0).map(|capture| capture.as_str().to_string()))
    {
        if let Some(date) = normalize_date_string(&candidate) {
            return Some(date);
        }
    }
    None
}

fn extract_last_amount(text: &str) -> Option<String> {
    amount_regex()
        .captures_iter(text)
        .filter_map(|captures| captures.get(1).map(|capture| capture.as_str().to_string()))
        .last()
        .and_then(|value| normalize_amount_string(&value))
}

fn extract_percentage(text: &str) -> Option<String> {
    percentage_regex().captures(text).and_then(|captures| {
        captures
            .get(1)
            .map(|capture| format!("{}%", capture.as_str()))
    })
}

fn extract_code_after_label(text: &str, label: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find(label)?;
    let value = text[start + label.len()..].trim();
    let value = value.trim_start_matches([':', ' ']);
    if value.is_empty() {
        None
    } else {
        Some(normalize_whitespace(value))
    }
}

fn date_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?x)
            \b(
                \d{4}-\d{2}-\d{2} |
                \d{4}/\d{2}/\d{2} |
                \d{2}/\d{2}/\d{4} |
                \d{2}\.\d{2}\.\d{4} |
                \d{1,2}\s+[A-Za-z]{3,9}\s+\d{4} |
                [A-Za-z]{3,9}\s+\d{1,2}\s+\d{4} |
                [A-Za-z]{3,9}\s+\d{1,2},\s+\d{4}
            )\b",
        )
        .expect("date regex should compile")
    })
}

fn amount_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)(\d[\d\s.,]*[.,]\d{2})").expect("amount regex should compile")
    })
}

fn percentage_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"(\d{1,2})\s*%").expect("percentage regex should compile"))
}

fn service_period_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(?i)service period\s+([A-Za-z]{3,9}\s+\d{1,2}(?:,\s*|\s+)\d{4})\s*[-–]\s*([A-Za-z]{3,9}\s+\d{1,2}(?:,\s*|\s+)\d{4})",
        )
        .expect("service period regex should compile")
    })
}

fn run_command_text(
    program: &str,
    args: &[&str],
    sample: &ReceiptSample,
    engine: &'static str,
) -> Result<String, ReceiptExtractorError> {
    let output = Command::new(program).args(args).output();
    match output {
        Ok(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
        Ok(output) => Err(ReceiptExtractorError::EngineFailed {
            engine,
            path: sample.document_path.display().to_string(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(ReceiptExtractorError::DependencyUnavailable {
                engine,
                message: format!("{program} is not installed"),
            })
        }
        Err(error) => Err(ReceiptExtractorError::EngineFailed {
            engine,
            path: sample.document_path.display().to_string(),
            message: error.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_name_extractor_uses_stem_tokens() {
        let sample = ReceiptSample {
            fixture: ReceiptReference {
                schema_version: 1,
                sample_id: "receipt-anthropic".to_string(),
                document_file: "receipt-anthropic-pbc-max-plan-2026-04-11.pdf".to_string(),
                original_file_name: String::new(),
                document_type: "receipt".to_string(),
                capture_type: "digital-pdf".to_string(),
                expense_candidate: true,
                related_document_file: String::new(),
                reference_status: "draft".to_string(),
                expected: ReceiptExpectedFields::default(),
                notes: vec![],
            },
            document_path: PathBuf::from(
                "data/receipts/receipt-anthropic-pbc-max-plan-2026-04-11.pdf",
            ),
            reference_path: PathBuf::from(
                "data/receipts/receipt-anthropic-pbc-max-plan-2026-04-11.reference.yaml",
            ),
        };

        let output = canonical_name_extract(&sample);
        assert_eq!(
            output.fields.get("merchant").map(String::as_str),
            Some("Anthropic Pbc")
        );
        assert_eq!(
            output.fields.get("issue_date").map(String::as_str),
            Some("2026-04-11")
        );
    }

    #[test]
    fn benchmark_normalizes_amounts_and_dates() {
        let reference = ReceiptExpectedFields {
            issue_date: "2026-04-10".to_string(),
            total: "995.00".to_string(),
            ..ReceiptExpectedFields::default()
        };
        let output = ExtractionOutput {
            engine: "test".to_string(),
            implementation: "test".to_string(),
            sample_id: "sample".to_string(),
            fields: BTreeMap::from([
                ("issue_date".to_string(), "10 April 2026".to_string()),
                ("total".to_string(), "995,00 kr".to_string()),
            ]),
            raw_text: None,
            warnings: vec![],
            metadata: BTreeMap::new(),
        };

        let benchmark = benchmark_output(&reference, &output);
        assert_eq!(benchmark.matched_fields, 2);
        assert!(benchmark.missing_fields.is_empty());
        assert!(benchmark.mismatched_fields.is_empty());
    }

    #[test]
    fn merge_text_fields_extracts_temporal_invoice_basics() {
        let text = "Invoice\nInvoice number U4HCRSHE-0006\nDate of issue April 2, 2026\nDate due May 2, 2026\nService period Mar 01 2026 - Mar 31 2026\nTemporal Technologies Inc.\n$0.00 USD due May 2, 2026";
        let mut fields = BTreeMap::new();
        merge_text_fields(&mut fields, text);
        assert_eq!(
            fields.get("invoice_number").map(String::as_str),
            Some("U4HCRSHE-0006")
        );
        assert_eq!(
            fields.get("issue_date").map(String::as_str),
            Some("2026-04-02")
        );
        assert_eq!(fields.get("currency").map(String::as_str), Some("USD"));
        assert_eq!(fields.get("total").map(String::as_str), Some("0.00"));
    }
}
