# Converge Integration Plan

## Immediate Pressure Tests

The CRM kernel should assume near-term integration with:

- `converge-analytics` for usage funnels, conversion cohorts, account scoring, and behavioral segmentation
- `converge-optimization` for lead routing, follow-up prioritization, queue balancing, and constrained work allocation
- `converge-llm` as the current ML-adjacent reasoning kernel in `../converge`

There is no `converge-ml` crate in the adjacent workspace snapshot. The closest current building blocks are `converge-analytics` and `converge-llm`.

## Runtime Modules To Expect

- `linkedin-scan`: governed profile and company signal ingestion
- `website-usage-ingest`: first-party behavioral events from `www.converge.zone`
- `lead-routing`: optimization-backed assignment and prioritization
- `account-fit-scoring`: analytics or ML-backed ranking over organizations and people

## Confirmed Converge Assets

- `../converge/crates/runtime/templates/linkedin-research.yaml`
- `../converge/crates/provider/src/linkedin.rs`
- `../converge/crates/application/src/packs.rs`

The LinkedIn pieces exist, but the provider is still a placeholder. Treat it as an integration seam, not a production ingestion path yet.

## Website Usage Ingestion

The site is at `/Users/kpernyer/dev/brand/www.converge.zone`.

Observed stack and surfaces:

- React 19 + Vite + TypeScript
- first-party event batching in `src/app/analytics/client.ts`
- Firebase Cloud Functions endpoints for `analyticsIngest` and `analyticsAggregates`

The repo still contains Plausible references, but the intended architecture is to have Plausible turned off and carry only the first-party event pipeline.

Observed first-party event types already emitted by the site:

- `session_start`
- `page_view`
- `page_scroll_milestone`
- `page_summary`
- `link_click`

Observed payload dimensions worth preserving in the CRM substrate:

- `sessionId`
- `pageViewId`
- `path` and `fromPath`
- campaign parameters such as `utm_*`, `gclid`, `fbclid`, and `ref`
- device and viewport class
- referrer host and same-origin status
- content kind and content id
- engagement metrics like visible time, active time, and max scroll depth

CRM consequence:

- ingest raw first-party events into a usage-signal lane
- correlate anonymous sessions to people and organizations when identity is later known
- expose aggregates back into account and contact summaries
- feed behavioral cohorts into `converge-analytics`
- feed prioritization signals into `converge-optimization`
- do not depend on Plausible as a required analytics vendor or source of truth

## Analytical Format Direction

When the analytical path moves beyond the in-memory scaffold, use Parquet as the batch and interchange format:

- land `website-usage-ingest` output as Parquet instead of raw JSON blobs
- let Polars-backed agents read Parquet directly for feature extraction
- export append-only audit and timeline history to Parquet for long-range analytical queries
- keep LanceDB and semantic retrieval Arrow-native, with Parquet as the handoff format where persistence is needed

This should stay separate from the SurrealDB transactional projection path. The likely shape in `crm-storage` is Parquet-aware export/import or a distinct analytical store boundary, not a single store trying to do both jobs.

## Architectural Consequence

This is not just a CRM with analytics bolted on. It is:

- CRM substrate for durable business state
- telemetry substrate for behavioral signals
- Converge runtime surface for governed agent action
- optimization and analytics hooks for prioritization and policy-safe automation

## Phase 1 Billing Bridge

The first live business integration path should be:

`converge-runtime billing webhook -> normalized billing event -> crm-server billing ingress -> ExecuteTruth -> CRM projection`

Current CRM-side boundary:

- `POST /v1/integrations/billing/events`
- bearer auth via `CRM_BILLING_INGRESS_TOKEN`
- idempotent delivery keyed by explicit `idempotency_key` or `source:truth:event_id`

Normalized event map:

- `prepaid_top_up_settled` -> `refill-prepaid-ai-credits`
- `subscription_activation_requested` -> `activate-subscription`
- `subscription_payment_failed` -> `suspend-service-on-payment-failure`
- `ledger_reconciliation_requested` -> `reconcile-model-usage-against-customer-ledger`

This keeps provider-specific webhook parsing in runtime adapters while CRM stays responsible for business-state mutation, approvals, and honest stopping.
