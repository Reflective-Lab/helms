use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BillingEventKind {
    PrepaidTopUpSettled,
    SubscriptionActivationRequested,
    SubscriptionPaymentFailed,
    LedgerReconciliationRequested,
}

impl BillingEventKind {
    #[must_use]
    pub fn truth_key(&self) -> &'static str {
        match self {
            Self::PrepaidTopUpSettled => "refill-prepaid-ai-credits",
            Self::SubscriptionActivationRequested => "activate-subscription",
            Self::SubscriptionPaymentFailed => "suspend-service-on-payment-failure",
            Self::LedgerReconciliationRequested => "reconcile-model-usage-against-customer-ledger",
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContractParseError {
    #[error("missing required input: {0}")]
    MissingInput(&'static str),
    #[error("invalid uuid for {field}: {value}")]
    InvalidUuid { field: &'static str, value: String },
    #[error("invalid integer for {field}: {value}")]
    InvalidInteger { field: &'static str, value: String },
    #[error("invalid datetime for {field}: {value}")]
    InvalidDateTime { field: &'static str, value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeExecutionScope {
    #[serde(default)]
    pub context_scope_id: Option<String>,
    #[serde(default)]
    pub resume_context: bool,
}

impl RuntimeExecutionScope {
    #[must_use]
    pub fn context_scope_id_or(&self, fallback: impl FnOnce() -> String) -> String {
        self.context_scope_id.clone().unwrap_or_else(fallback)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CreditPaymentStatus {
    Confirmed,
    Pending,
    Failed,
    Unknown,
}

impl CreditPaymentStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Pending => "pending",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionFailureStatus {
    Failed,
    Overdue,
}

impl SubscriptionFailureStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::Overdue => "overdue",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateSubscriptionInput {
    pub subscription_id: Uuid,
    #[serde(default)]
    pub catalog_item_id: Option<Uuid>,
    #[serde(default)]
    pub opening_balance_minor: Option<i64>,
    #[serde(default)]
    pub opening_balance_currency_code: Option<String>,
    #[serde(default)]
    pub force_manual_review: bool,
    #[serde(default)]
    pub manual_review_reason: Option<String>,
    #[serde(default)]
    pub owner_user_id: Option<String>,
    #[serde(default)]
    pub workflow_title: Option<String>,
    #[serde(default)]
    pub scope: RuntimeExecutionScope,
}

impl ActivateSubscriptionInput {
    #[must_use]
    pub fn truth_key(&self) -> &'static str {
        BillingEventKind::SubscriptionActivationRequested.truth_key()
    }

    #[must_use]
    pub fn context_scope_id(&self) -> String {
        self.scope.context_scope_id_or(|| {
            format!(
                "truth:{}:subscription:{}",
                self.truth_key(),
                self.subscription_id
            )
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefillPrepaidAiCreditsInput {
    pub subscription_id: Uuid,
    pub top_up_amount_minor: i64,
    pub payment_reference: String,
    pub payment_status: CreditPaymentStatus,
    #[serde(default)]
    pub risk_signal: bool,
    #[serde(default)]
    pub force_manual_review: bool,
    #[serde(default)]
    pub manual_review_reason: Option<String>,
    #[serde(default)]
    pub scope: RuntimeExecutionScope,
}

impl RefillPrepaidAiCreditsInput {
    #[must_use]
    pub fn truth_key(&self) -> &'static str {
        BillingEventKind::PrepaidTopUpSettled.truth_key()
    }

    #[must_use]
    pub fn context_scope_id(&self) -> String {
        self.scope.context_scope_id_or(|| {
            format!(
                "truth:{}:payment:{}",
                self.truth_key(),
                self.payment_reference
            )
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspendServiceOnPaymentFailureInput {
    pub subscription_id: Uuid,
    pub payment_status: SubscriptionFailureStatus,
    #[serde(default)]
    pub days_overdue: i64,
    #[serde(default)]
    pub grace_days: Option<i64>,
    #[serde(default)]
    pub strategic_account: Option<bool>,
    #[serde(default)]
    pub force_manual_review: bool,
    #[serde(default)]
    pub manual_review_reason: Option<String>,
    #[serde(default)]
    pub scope: RuntimeExecutionScope,
}

impl SuspendServiceOnPaymentFailureInput {
    #[must_use]
    pub fn truth_key(&self) -> &'static str {
        BillingEventKind::SubscriptionPaymentFailed.truth_key()
    }

    #[must_use]
    pub fn context_scope_id(&self) -> String {
        self.scope.context_scope_id_or(|| {
            format!(
                "truth:{}:subscription:{}",
                self.truth_key(),
                self.subscription_id
            )
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconcileModelUsageAgainstCustomerLedgerInput {
    pub subscription_id: Uuid,
    pub usage_burn_minor: i64,
    pub provider_settled_minor: i64,
    #[serde(default)]
    pub provider_reference: Option<String>,
    #[serde(default)]
    pub provider_name: Option<String>,
    #[serde(default)]
    pub provider_status: Option<String>,
    #[serde(default)]
    pub threshold_minor: Option<i64>,
    #[serde(default)]
    pub meter_name: Option<String>,
    #[serde(default)]
    pub period_label: Option<String>,
    #[serde(default)]
    pub scope: RuntimeExecutionScope,
}

impl ReconcileModelUsageAgainstCustomerLedgerInput {
    #[must_use]
    pub fn truth_key(&self) -> &'static str {
        BillingEventKind::LedgerReconciliationRequested.truth_key()
    }

    #[must_use]
    pub fn context_scope_id(&self) -> String {
        self.scope.context_scope_id_or(|| {
            format!(
                "truth:{}:subscription:{}",
                self.truth_key(),
                self.subscription_id
            )
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "truth_key", rename_all = "kebab-case")]
pub enum BillingTruthPayload {
    ActivateSubscription(ActivateSubscriptionInput),
    RefillPrepaidAiCredits(RefillPrepaidAiCreditsInput),
    SuspendServiceOnPaymentFailure(SuspendServiceOnPaymentFailureInput),
    ReconcileModelUsageAgainstCustomerLedger(ReconcileModelUsageAgainstCustomerLedgerInput),
}

impl BillingTruthPayload {
    #[must_use]
    pub fn truth_key(&self) -> &'static str {
        match self {
            Self::ActivateSubscription(input) => input.truth_key(),
            Self::RefillPrepaidAiCredits(input) => input.truth_key(),
            Self::SuspendServiceOnPaymentFailure(input) => input.truth_key(),
            Self::ReconcileModelUsageAgainstCustomerLedger(input) => input.truth_key(),
        }
    }

    #[must_use]
    pub fn context_scope_id(&self) -> String {
        match self {
            Self::ActivateSubscription(input) => input.context_scope_id(),
            Self::RefillPrepaidAiCredits(input) => input.context_scope_id(),
            Self::SuspendServiceOnPaymentFailure(input) => input.context_scope_id(),
            Self::ReconcileModelUsageAgainstCustomerLedger(input) => input.context_scope_id(),
        }
    }

    #[must_use]
    pub fn resume_context(&self) -> bool {
        match self {
            Self::ActivateSubscription(input) => input.scope.resume_context,
            Self::RefillPrepaidAiCredits(input) => input.scope.resume_context,
            Self::SuspendServiceOnPaymentFailure(input) => input.scope.resume_context,
            Self::ReconcileModelUsageAgainstCustomerLedger(input) => input.scope.resume_context,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedBillingEvent {
    pub source: String,
    pub event_id: String,
    pub event_kind: BillingEventKind,
    pub idempotency_key: String,
    pub persist_projection: bool,
    pub received_at: DateTime<Utc>,
    pub payload: BillingTruthPayload,
}

impl NormalizedBillingEvent {
    #[must_use]
    pub fn truth_key(&self) -> &'static str {
        self.payload.truth_key()
    }

    #[must_use]
    pub fn context_scope_id(&self) -> String {
        self.payload.context_scope_id()
    }
}

impl TryFrom<&HashMap<String, String>> for ActivateSubscriptionInput {
    type Error = ContractParseError;

    fn try_from(inputs: &HashMap<String, String>) -> Result<Self, Self::Error> {
        Ok(Self {
            subscription_id: required_uuid(inputs, "subscription_id")?,
            catalog_item_id: optional_uuid(inputs, "catalog_item_id")?,
            opening_balance_minor: optional_i64(inputs, "opening_balance_minor")?,
            opening_balance_currency_code: optional_string(inputs, "opening_balance_currency_code"),
            force_manual_review: optional_bool(inputs, "force_manual_review"),
            manual_review_reason: optional_string(inputs, "manual_review_reason"),
            owner_user_id: optional_string(inputs, "owner_user_id"),
            workflow_title: optional_string(inputs, "workflow_title"),
            scope: runtime_scope_from_inputs(inputs),
        })
    }
}

impl TryFrom<&HashMap<String, String>> for RefillPrepaidAiCreditsInput {
    type Error = ContractParseError;

    fn try_from(inputs: &HashMap<String, String>) -> Result<Self, Self::Error> {
        Ok(Self {
            subscription_id: required_uuid(inputs, "subscription_id")?,
            top_up_amount_minor: required_i64(inputs, "top_up_amount_minor")
                .or_else(|_| required_i64(inputs, "amount_minor"))?,
            payment_reference: required_string(inputs, "payment_reference")?,
            payment_status: credit_payment_status_from_inputs(inputs),
            risk_signal: optional_bool(inputs, "risk_signal"),
            force_manual_review: optional_bool(inputs, "force_manual_review"),
            manual_review_reason: optional_string(inputs, "manual_review_reason"),
            scope: runtime_scope_from_inputs(inputs),
        })
    }
}

impl TryFrom<&HashMap<String, String>> for SuspendServiceOnPaymentFailureInput {
    type Error = ContractParseError;

    fn try_from(inputs: &HashMap<String, String>) -> Result<Self, Self::Error> {
        Ok(Self {
            subscription_id: required_uuid(inputs, "subscription_id")?,
            payment_status: subscription_failure_status(required_string(
                inputs,
                "payment_status",
            )?)?,
            days_overdue: optional_i64(inputs, "days_overdue")?
                .unwrap_or_default()
                .max(0),
            grace_days: optional_i64(inputs, "grace_days")?,
            strategic_account: optional_bool_value(inputs, "strategic_account"),
            force_manual_review: optional_bool(inputs, "force_manual_review"),
            manual_review_reason: optional_string(inputs, "manual_review_reason"),
            scope: runtime_scope_from_inputs(inputs),
        })
    }
}

impl TryFrom<&HashMap<String, String>> for ReconcileModelUsageAgainstCustomerLedgerInput {
    type Error = ContractParseError;

    fn try_from(inputs: &HashMap<String, String>) -> Result<Self, Self::Error> {
        Ok(Self {
            subscription_id: required_uuid(inputs, "subscription_id")?,
            usage_burn_minor: required_i64(inputs, "usage_burn_minor")?,
            provider_settled_minor: required_i64(inputs, "provider_settled_minor")?,
            provider_reference: optional_string(inputs, "provider_reference"),
            provider_name: optional_string(inputs, "provider_name"),
            provider_status: optional_string(inputs, "provider_status"),
            threshold_minor: optional_i64(inputs, "threshold_minor")?,
            meter_name: optional_string(inputs, "meter_name"),
            period_label: optional_string(inputs, "period_label"),
            scope: runtime_scope_from_inputs(inputs),
        })
    }
}

pub fn required_string(
    inputs: &HashMap<String, String>,
    key: &'static str,
) -> Result<String, ContractParseError> {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or(ContractParseError::MissingInput(key))
}

pub fn optional_string(inputs: &HashMap<String, String>, key: &str) -> Option<String> {
    inputs
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn required_uuid(
    inputs: &HashMap<String, String>,
    key: &'static str,
) -> Result<Uuid, ContractParseError> {
    let value = required_string(inputs, key)?;
    Uuid::parse_str(&value).map_err(|_| ContractParseError::InvalidUuid { field: key, value })
}

pub fn optional_uuid(
    inputs: &HashMap<String, String>,
    key: &'static str,
) -> Result<Option<Uuid>, ContractParseError> {
    optional_string(inputs, key)
        .map(|value| {
            Uuid::parse_str(&value)
                .map_err(|_| ContractParseError::InvalidUuid { field: key, value })
        })
        .transpose()
}

pub fn required_i64(
    inputs: &HashMap<String, String>,
    key: &'static str,
) -> Result<i64, ContractParseError> {
    let value = required_string(inputs, key)?;
    value
        .parse::<i64>()
        .map_err(|_| ContractParseError::InvalidInteger { field: key, value })
}

pub fn optional_i64(
    inputs: &HashMap<String, String>,
    key: &'static str,
) -> Result<Option<i64>, ContractParseError> {
    optional_string(inputs, key)
        .map(|value| {
            value
                .parse::<i64>()
                .map_err(|_| ContractParseError::InvalidInteger { field: key, value })
        })
        .transpose()
}

pub fn required_datetime(
    inputs: &HashMap<String, String>,
    key: &'static str,
) -> Result<DateTime<Utc>, ContractParseError> {
    let value = required_string(inputs, key)?;
    DateTime::parse_from_rfc3339(&value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| ContractParseError::InvalidDateTime { field: key, value })
}

pub fn optional_bool(inputs: &HashMap<String, String>, key: &str) -> bool {
    optional_bool_value(inputs, key).unwrap_or(false)
}

pub fn optional_bool_value(inputs: &HashMap<String, String>, key: &str) -> Option<bool> {
    optional_string(inputs, key).and_then(|value| match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    })
}

pub fn runtime_scope_from_inputs(inputs: &HashMap<String, String>) -> RuntimeExecutionScope {
    RuntimeExecutionScope {
        context_scope_id: optional_string(inputs, "context_scope_id"),
        resume_context: optional_bool(inputs, "resume_context"),
    }
}

pub fn credit_payment_status_from_inputs(inputs: &HashMap<String, String>) -> CreditPaymentStatus {
    if let Some(confirmed) = optional_bool_value(inputs, "payment_confirmed") {
        return if confirmed {
            CreditPaymentStatus::Confirmed
        } else {
            CreditPaymentStatus::Pending
        };
    }

    match optional_string(inputs, "payment_status")
        .unwrap_or_else(|| "unknown".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "confirmed" | "paid" | "settled" => CreditPaymentStatus::Confirmed,
        "pending" | "authorized" | "processing" => CreditPaymentStatus::Pending,
        "failed" | "declined" | "overdue" => CreditPaymentStatus::Failed,
        _ => CreditPaymentStatus::Unknown,
    }
}

pub fn subscription_failure_status(
    value: String,
) -> Result<SubscriptionFailureStatus, ContractParseError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "failed" | "declined" | "payment_failed" => Ok(SubscriptionFailureStatus::Failed),
        "overdue" | "past_due" => Ok(SubscriptionFailureStatus::Overdue),
        _ => Err(ContractParseError::InvalidInteger {
            field: "payment_status",
            value,
        }),
    }
}
