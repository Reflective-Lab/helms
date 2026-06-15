# Quorum Sense Review — Helm Architect

Date: 2026-06-15
Reviewer marker: `HELM_ARCHITECT`
Scope: Helm, Runtime Runway, Commerce Rails, and `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense`

## HELM_ARCHITECT Summary

Quorum should own inquiry semantics: contracts, signals, hypotheses, probes, evidence topology, quorum status/outcome, process receipts, and product-specific UX.

Quorum should not own common app hosting, Cloud Run packaging, auth/bootstrap, commerce entitlement semantics, Stripe wiring, shared app shell, or Helm operator-control runtime behavior. Those belong in Runtime Runway, Commerce Rails, or Helm.

## HELM_ARCHITECT Findings

1. Runtime substrate is being forked in Quorum.

   Runtime Runway explicitly owns host/deploy/auth/storage/telemetry, but Quorum has its own Cloud Run provisioner, Cloud Build config, and Dockerfile cloning platform repos directly. The server also manually initializes telemetry, storage, auth, Commerce Rails, account routes, and SPA serving. This belongs in Runtime Runway.

   Evidence:
   - `/Users/kpernyer/dev/reflective/runtime-runway/kb/Architecture/App Execution Container.md`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/deploy/cloud-run-provision.sh`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/cloudbuild.yaml`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/deploy/backend/Dockerfile.cloudrun`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/crates/quorum-server/src/main.rs`

2. Commerce Rails boundary is leaking into the app.

   Commerce Rails owns subscriptions, entitlements, billing, ledger, and provider semantics. Quorum imports `commerce-rails-stripe` directly, constructs `CommerceRails`, implements entitlement modes, hard-codes the `quorum` entitlement and signup URL, and deploys Stripe price/secrets itself. The app should consume an entitlement result or claim, not know Stripe or Commerce Rails provider wiring.

   Evidence:
   - `/Users/kpernyer/dev/reflective/commerce-rails/kb/Contracts/Commerce Rail Surface.md`
   - `/Users/kpernyer/dev/reflective/commerce-rails/kb/Architecture/Operating Authority Boundary.md`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/crates/quorum-server/Cargo.toml`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/crates/quorum-server/src/main.rs`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/deploy/cloud-run-provision.sh`

3. Helm is mounted, but not live enough to count as the real operator surface.

   Quorum mounts `OperatorControlModule::new` and `GovernedJobsModule::new`, but those constructors intentionally use empty/default wiring. Operator-control pipeline routes can return 501 without truth registration, and governed jobs return 501 for all truth keys until real state is supplied. Meanwhile Quorum's actual Helm handoff adapter is only used in tests, not by the server. Either wire the live Quorum readiness packet into Helm, or keep the mount marked planned.

   Evidence:
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/crates/quorum-server/src/main.rs`
   - `/Users/kpernyer/dev/reflective/bedrock-platform/helms/crates/helm-operator-control/src/lib.rs`
   - `/Users/kpernyer/dev/reflective/bedrock-platform/helms/crates/helm-governed-jobs/src/lib.rs`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/crates/quorum-app/src/helm_adapter.rs`

4. Test-only proposal admission is exposed over HTTP.

   The evidence and authority routes call `apply_proposals_for_test`; the app marks that method `TEST-ONLY` and `#[doc(hidden)]`. This needs a production admission API through the Quorum/Converge path or should be gated out of production.

   Evidence:
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/crates/quorum-server/src/main.rs`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/crates/quorum-app/src/lib.rs`

5. Multi-instance write ownership is not enforced.

   The app defaults to local in-process ownership/live bus/idempotency. The `SessionOwnership` trait says callers decide what to do with ownership, and current write paths do not use it. Cloud Run deploy allows `--max-instances=3`. Until Runtime Runway provides a lease/owner gate, set max instances to 1 or enforce ownership on every mutating route.

   Evidence:
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/crates/quorum-app/src/lib.rs`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/crates/quorum-app/src/scale_seams.rs`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/deploy/cloud-run-provision.sh`

6. Common app shell and auth UX are being duplicated in Quorum.

   `MarqueeTopbar` is a portfolio app nav; `DesktopShell` includes generic app nav, subscription, config, and user management; Firebase auth/account bootstrap is app-local; entitlement redirect parsing is app-local. Shared shell/auth belongs in Runtime Runway, subscription/checkout widgets belong in Commerce Rails, and operator/trust components belong in Helm.

   Evidence:
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/apps/desktop/src/lib/MarqueeTopbar.svelte`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/apps/desktop/src/lib/DesktopShell.svelte`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/apps/desktop/src/lib/firebase-client.ts`
   - `/Users/kpernyer/dev/reflective/marquee-apps/quorum-sense/apps/desktop/src/lib/product-api.ts`

## HELM_ARCHITECT Ownership Split

Runtime Runway should take: Cloud Run recipes, container/base image, static SPA serving, auth/middleware, route prefix, health/status, secrets/env bootstrap, storage/event-log backend, telemetry, Firebase client/account bootstrap, session ownership lease.

Commerce Rails should take: app listings, plans/prices, Stripe/provider mapping, checkout/portal redirects, webhook semantics, entitlement grants, commercial receipts/reconciliation.

Helm should take: live `JobReadinessPacket`/`OperatorLedgerEntry` feed, operator-control routes, governed job stream wiring, HITL/action receipts, process-receipt rendering, and the no-domain-authority guardrails.

Quorum keeps: inquiry lifecycle, contract drafting, consent/anonymity, evidence topology, reachability, status/outcome, process receipt, `quorum://` subject refs, and product-specific UX.

## HELM_ARCHITECT Immediate Recommendation

Freeze new Quorum-local platform work, wire the live Helm handoff, replace `apply_proposals_for_test`, and move deployment/commerce/app-shell concerns upstream before Atlas copies this shape.

## HELM_ARCHITECT Self-Adjustment Before Reconciliation

Date: 2026-06-15

After reviewing Runtime Runway and Commerce Rails feedback against Helm's own code and docs, I adjust my stand as follows.

1. The core Helm boundary does not change.

   Helm is still the operator trust-transfer surface: readiness packets, operator ledgers, HITL/action receipts, governed-job surfaces, receipt views, and workbench contracts. Helm must not own runtime/deploy substrate, commercial authority, or Quorum's inquiry state machine.

2. I narrow the commerce criticism of Quorum.

   Commerce Rails is right that Quorum does not read Stripe directly, parse invoices, own subscription state, or implement its own webhook. The current code-level path through `CommerceRails::is_entitled(...)`, `AccountsState::with_commerce(...)`, and `runway_accounts::public_routes(...)` is the current CR/RR contract.

   The remaining concern is not "Quorum owns Stripe." It is: deploy-time Stripe/env wiring, error/redirect shape, and client entitlement UX still need an upstream CR/RR contract so the next app does not copy app-local glue.

3. I strengthen the critique of Helm's own commercial leftovers.

   The problem is bigger than the four `prio-*` commercial manifest crates. Helm still has active commercial state and truth execution paths in `application-kernel`, `workbench-backend`, and `truth-catalog`:

   - `application-kernel` owns subscription, entitlement, credit-grant, payment-reference, and commercial ledger mutations;
   - `workbench-backend` executes `activate-subscription` and `refill-prepaid-ai-credits` by mutating that kernel state;
   - `truth-catalog` still frames subscription/payment/entitlement truths as Helm module touches;
   - `contracts/module-registry.yaml` and `kb/Architecture/Module Map.md` still describe commercial/revenue capabilities as Helm-owned capability modules.

   That was historical scaffolding before Commerce Rails existed. It is now wrong as platform direction. The adjustment is: these should be rehomed to Commerce Rails, converted into Helm operator projections over Commerce Rails contracts, or marked historical/disabled. Helm should not merely rename `prio-subscriptions`; it should stop claiming commercial domain ownership.

4. I soften the Runtime Runway criticism.

   Quorum using `RunwayAppHost::builder(packet)` is the correct direction and should be recognized as the reference shape with known gaps. My Round 1 phrase "runtime substrate is being forked" was too broad. The accurate statement is: Runtime Runway has not yet provided enough primitives, so Quorum still carries local glue for deploy, SPA serving, shell/auth UX, module auth, and session ownership.

5. I keep the Helm mount/status criticism.

   `helm.operator-control` and `helm.governed-jobs` routes are physically mounted, but they are default shells for Quorum. `mount_kind: planned` is more truthful if the field means live capability. The platform needs either richer status (`mounted` + `shell|live`) or a rule that mounted means live.

6. I adjust the implementation ask.

   Round 3 should not tell implementors to freeze Quorum product work. It should say: freeze new Quorum-local platform duplication. Continue Quorum domain work, but do not add more local runtime, commerce, app shell, or Helm-readiness substitutes.

## Peer Review Comments

### RUNTIME_RUNWAY_ARCHITECT

Date: 2026-06-15 (Round 2)
Master file with all three reviews + cross-comments: `/Users/kpernyer/dev/reflective/REVIEW_quorum-sense_2026-06-15.md`

**Finding 1 — Runtime substrate is being forked in Quorum.** Partially refute. The host is NOT forked: `quorum-server/src/main.rs:1898–1911` uses `RunwayAppHost::builder().with_storage(...).mount(...).build().await?.serve()` — that is the canonical pattern, not a fork. What you cite as "manually initializes telemetry, storage, auth, Commerce Rails, account routes, SPA serving" is the app *configuring* the host's typed inputs, not bypassing it. What IS true in your finding, narrower: (a) `deploy/cloud-run-provision.sh` and `cloudbuild.yaml` are app-local copies of what RR should template; (b) SPA static-serve at `main.rs:22, 1893–1908` is hand-rolled and should be a `RunwayAppHostBuilder::with_spa(...)` primitive; (c) shell duplication (your finding #6) is a separate RR-owned crate. Accepting the direction but rejecting the framing — quorum-server is the cleanest substrate consumer we have shipped. Please do not let this finding land as "quorum is forking RR" in Round 3; reframe as "RR has not yet templated the deploy and shell surface, so quorum had to."

**Finding 2 — Commerce Rails boundary is leaking into the app.** Refute on code, accept on deploy. Quorum does NOT know Stripe at the code level. `[CR-ARCH]` and I both verified the same evidence: no direct `stripe-rust` / `async-stripe` import; only `commerce-rails-stripe` (which is a CR-owned provider adapter, not Stripe); the entitlement gate calls `CommerceRails::is_entitled(uid, "quorum")` at `main.rs:1469-1480`; the webhook is delegated to `runway_accounts::public_routes` with a shared `CommerceRails` Arc. The crate name `commerce-rails-stripe` is the CR adapter, not a Stripe coupling — quorum's code surface is `CommerceRails`, never `stripe::*`. Where you ARE right: the deploy script `deploy/cloud-run-provision.sh` carries `STRIPE_PRICE_*` env vars and `STRIPE_SECRET_KEY` / `STRIPE_WEBHOOK_SECRET` references — that is deploy-time Stripe coupling, and it IS app-local leakage. Per CR's recommendation #5, price IDs and webhook secrets belong in CR's provider config, surfaced to the app as a single opaque entitlement check. Filing as a new RR finding **D6** in the master file.

**Finding 3 — Helm is mounted, but not live enough.** Strong accept. This sharpens my own Round 1 D2 finding ("planned-vs-mounted lie"). Your evidence flips the resolution: the manifest's `"planned"` is *more* accurate than the code's `.mount()` because the modules are 501-stubs without truth registration. Revised action: back out the `.mount()` calls until the live `JobReadinessPacket` is wired through `quorum-app::helm_adapter` (currently test-only), OR ship the live wiring now. Owners for Round 3: HELMS-ARCH for the live packet contract; quorum-app for the wiring; RR for the manifest verifier (my D1) that would have caught this automatically.

**Finding 4 — Test-only proposal admission exposed over HTTP.** Strong accept. I missed this in Round 1. This is severity **A** (Must fix now) in our ledger taxonomy, not B. `#[doc(hidden)]` and the `TEST-ONLY` doc comment are documentation, not enforcement. RR will add a generic check in `runway-app-host::verify_manifest` (the D1 verifier) that fails the build if any route handler resolves to a symbol whose name contains `for_test` / `_test` / `test_only`. Drift check is RR's; the immediate fix is quorum's.

**Finding 5 — Multi-instance write ownership not enforced.** Accept and own. This is RR scope and a Round 1 miss on my side. `runway-app-host` or `runway-storage` owes a `SessionOwnership` primitive: lease per `(org_id, app_id, session_id)` with TTL, renewal, and steal semantics. Until that lands, any marquee app at `--max-instances > 1` risks split-brain on every mutating route. Filing as my new **D5** in the master file. Severity **A**. Quorum's interim mitigation: pin `--max-instances=1` in `cloud-run-provision.sh` and document the constraint.

**Finding 6 — Common app shell and auth UX duplicated.** Strong accept. This escalates my Round 1 D3 from "SPA static-serve primitive" to "shared frontend shell crate." New RR scope: `runway-app-shell` (frontend package + Rust handlers). Splitting D3 into D3a (server-side SPA primitive in `runway-app-host`) and D3b (frontend shell crate). The subscription/checkout widget split you mention is a CR call — defer to `[CR-ARCH]` for whether `commerce-rails-ui` exists or needs to be created.

**On your ownership-split table.** Fully aligned. RR is adopting it verbatim into `runtime-runway/kb/Architecture/App Execution Container.md` once CR signs the CR row. One nuance on "Firebase client/account bootstrap" — yes, RR, but the commercial claim minting flow runs RR → CR → runway-accounts: the claim *source of truth* is CR's entitlement decision, the claim *minting* is RR's identity surface. The seam at `main.rs:1557-1566` is the right shape; just be explicit about who decides vs. who emits.

**On your immediate recommendation ("freeze new Quorum-local platform work").** Accept the spirit, soften the framing for Round 3. We are not freezing quorum's *product* work — that would confuse incentives. We are freezing **new quorum-local platform code that duplicates RR / CR / Helms**. The existing `deploy/`, SPA static-serve, frontend shell, and deploy-script Stripe vars stay frozen as-is and migrate upstream in priority order: D5 (ownership lease, severity A) → D4 (apply_proposals_for_test, severity A) → D6 (deploy-time Stripe vars) → D3a (SPA primitive) → D3b (shell crate) → D1 (manifest verifier).

**On the `prio-*` modules in `bedrock-platform/helms/crates/`** that CR-ARCH flagged: HELMS-ARCH Round 1 does not address them. CR-ARCH considers it the strongest upstream finding. RR backs CR's rule (commercial state belongs to commerce-rails, not helms). Please address explicitly in Round 2 — if `prio-subscriptions` / `prio-entitlements` / `prio-payments` / `prio-metering` are legitimate operator-UI projections with no domain state, say so with evidence; otherwise demote or delete. RR will add a drift check at `runway-app-host` build time that fails any marquee app importing those crate names.

— RUNTIME_RUNWAY_ARCHITECT, 2026-06-15

### COMMERCE_RAILS_ARCHITECT

Reviewer marker: `COMMERCE_RAILS_ARCHITECT`
Date: 2026-06-15 (Round 2)
Master file with all three reviews + cross-comments + Round 3: `/Users/kpernyer/dev/reflective/REVIEW_quorum-sense_2026-06-15.md`

**On HELMS-ARCH's self-adjustment (lines 88–126):** received and accepted. The reconciliation is substantial — HELMS has moved further toward the CR position than CR Round 1 asked for. Specific responses:

**Section 2 (narrowing the commerce criticism of Quorum):** accepted. CR refute on code, partial accept on deploy is the right read. RR-ARCH filed the deploy-side leak as D6 in the master; CR owns the fix via `QF-2026-06-15-CR-04` (new `commerce-rails-deploy` contract).

**Section 3 (strengthening Helm's own commercial leftovers):** **strong accept, and noted as an expansion of CR's Round-1 D-CR-1.** The problem is bigger than the four `prio-*` crates. The new sites HELMS names are not on CR's radar from Round 1; CR will track them under the same migration:
- `application-kernel` owning subscription / entitlement / credit-grant / payment-reference / commercial-ledger mutations → **must move to Commerce-Rails or become a thin projection.** This is the most load-bearing of the new sites. CR needs to read the kernel's current public API before specifying the migration shape.
- `workbench-backend` executing `activate-subscription` and `refill-prepaid-ai-credits` → these are CR command handlers, not operator workbench actions. The HITL/approval surface stays in Helm; the state mutation moves to CR.
- `truth-catalog` still framing subscription/payment/entitlement truths as Helm module touches → CR-side truth shapes. Helm consumes them as projections, does not author them.
- `contracts/module-registry.yaml` and `kb/Architecture/Module Map.md` describing commercial/revenue capabilities as Helm-owned → documentation fix as the code moves.

HELMS's framing is exactly right: *"Helm should not merely rename `prio-subscriptions`; it should stop claiming commercial domain ownership."* Round 3 implementor message will reflect this.

**Section 4 (softening RR critique):** noted, no CR equities. The reframe ("RR has not yet templated the deploy and shell surface, so quorum had to") is the right shape.

**Section 5 (keep Helm mount/status criticism):** noted. CR has no equities on `mounted` vs `mounted+shell|live` semantics — HELMS's call.

**Section 6 (adjust implementation ask):** accepted. Round 3 says *freeze new quorum-local platform duplication, not product work*.

**On RR-ARCH's peer-review block above (lines 129–152):** CR has confirmed RR's reads independently. RR's line 150 challenge to HELMS on `prio-*` is now resolved by HELMS's section 3 self-adjustment.

### CR open questions for Round 3 (HELMS-ARCH input requested)

1. **Migration mechanic for `prio-*` + `application-kernel` + `workbench-backend` + `truth-catalog` commercial sites.** Options:
   - (a) Crate-by-crate move into `commerce-rails/crates/` with `#[deprecated]` re-exports in Helms for one release.
   - (b) In-place demotion: keep the crate names, gut the state, leave thin projection adapters that call CR by contract.
   - (c) Feature-flagged sunset: gate the legacy code behind `helms-legacy-commercial` with a dated removal milestone.
   CR has no strong preference — wants HELMS to pick based on impact on existing Helm consumers.

2. **Sequencing.** CR proposes: (i) publish `commerce-rails-deploy` env contract (`QF-CR-04`) → (ii) freeze new Helm commercial writes → (iii) migrate `application-kernel` commercial mutations → (iv) demote `prio-*` → (v) clean up registry/map docs. HELMS confirm or counter.

### CR commitments filed at the master file (full text there)

- `QF-2026-06-15-CR-02` (B) — reconcile M2 charter naming quorum-sense as integration driver.
- `QF-2026-06-15-CR-03` (A) — land `EntitlementStore` v2 on `runway-storage::DocumentStore`.
- `QF-2026-06-15-CR-04` (A) — publish `commerce-rails-deploy` env contract; quorum migrates onto it.
- `QF-2026-06-15-CR-05` (B) — create `commerce-rails-shell` crate (subscription/checkout widget).
- `QF-2026-06-15-CR-06` (A) — publish entitlement-change event contract; RR subscribes for claim refresh.
- `QF-2026-06-15-CR-07` (B) — extend entitlement projection with optional `checkout_url` / `portal_url` / `signup_url` (accepts HELMS Round-2 line 187 proposal).

Round 3 (coherent implementor message) will be drafted in the master file once HELMS-ARCH answers the migration-mechanic + sequencing questions above.

— `COMMERCE_RAILS_ARCHITECT`, 2026-06-15
