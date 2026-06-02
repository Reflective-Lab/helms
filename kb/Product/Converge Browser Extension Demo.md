# Converge.Zone — Browser Extension Demo

> **Target audience:** Whatfix (DAP company), mid-late April 2026
>
> **Thesis:** DAP instruments applications. Converge governs jobs.
> Together, you get guidance that knows the actual state of work.

---

## The Problem Whatfix Sees

Enterprise users are trapped in fragmented systems. They don't know where to click,
which system to open next, or whether someone (or something) already handled step 3.

Whatfix solves this with overlays: tooltips, walkthroughs, in-app guidance.

But the overlay doesn't know:
- What **job** the user is actually trying to complete
- What **state** that job is in across systems
- What an **agent** already did on their behalf
- What **policy** blocks the next step

Whatfix instruments the application. It doesn't see the work.

## What Converge Adds

Converge is a governance runtime for agent-driven work. It tracks:
- **Intent** — what job is being done, with success criteria and constraints
- **Facts** — what is known, proposed by agents, promoted through governance
- **Convergence** — how close the job is to done, and what's blocking it
- **Policy** — hard constraints and human-in-the-loop gates

A browser extension powered by Converge doesn't guide users through clicks.
It shows them the **state of their job** — regardless of which application they're in.

## Demo Scenario

### Setup

User is working on: **Onboard Acme Corp as a paying customer.**

This spans multiple systems (billing, CRM, provisioning). In a traditional
enterprise, the user would alt-tab between 3-5 apps, following a mental checklist.
Whatfix would overlay guidance on each app individually.

### The Extension

A Chrome side panel connected to Converge, showing:

```
┌─ Converge ────────────────────────────┐
│                                       │
│ Onboard Acme Corp                     │
│ ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │
│                                       │
│ ✓ Lead qualified                      │
│   Agent: routing-agent, 2 min ago     │
│   Confidence: 87%                     │
│                                       │
│ ✓ Subscription plan selected          │
│   Agent: commercial-agent             │
│   Plan: Growth — $4,200/mo            │
│                                       │
│ ✓ Payment confirmed                   │
│   Source: Stripe webhook              │
│   Amount: $4,200.00                   │
│                                       │
│ ⧖ Entitlements pending approval       │
│   $50,400 ARR exceeds auto-approve    │
│   Policy: top_up_requires_confirmed   │
│                                       │
│   [Approve]  [Escalate]  [Details]    │
│                                       │
│ ○ Workspace provisioning              │
│   Waiting on entitlement approval     │
│                                       │
│ Convergence: 3/5 criteria met         │
│ ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━ │
│                                       │
│ ↻ Live · Updated 4s ago              │
└───────────────────────────────────────┘
```

### The Punchline

The user didn't navigate to the right screen in the right app.
They didn't follow a walkthrough. They didn't need training.

They see the **job**, the **state**, and the **one thing that needs them**.

The extension works the same whether the tab behind it shows Salesforce,
SAP, a spreadsheet, or nothing at all — because the job state lives in
Converge, not in the application.

## Demo Flow (90 seconds)

1. **Open any web app** — Salesforce, Gmail, whatever. Doesn't matter.
2. **Open the Converge side panel** — shows active jobs for this user/account.
3. **Trigger a billing event** — `POST /v1/integrations/billing/events` with
   a prepaid top-up. Watch the side panel update in real time.
4. **Show the blocked step** — entitlement approval needed. Policy is visible.
   User clicks Approve in the side panel.
5. **Watch convergence complete** — remaining criteria met, job done.

One sentence after: *"Whatfix guides users through applications.
Converge shows them the state of their work. Imagine both together."*

## Technical Architecture

```
┌─────────────────────┐     ┌──────────────────────┐
│  Chrome Extension    │────▶│ application-server   │
│  (Side Panel)        │◀────│  port 8081           │
│                      │     │                      │
│  - Svelte UI         │     │  Existing endpoints: │
│  - Polls job state   │     │  GET  /v1/truths     │
│  - Renders timeline  │     │  POST /v1/truths/exe │
│  - Approval actions  │     │  GET  /v1/timeline   │
│  - SSE or polling    │     │  GET  /v1/workflow    │
└─────────────────────┘     │  POST /v1/billing     │
                            └──────────────────────┘
                                      │
                            ┌─────────▼────────────┐
                            │  Converge Runtime     │
                            │  (governance engine)  │
                            │                      │
                            │  - Intent + criteria  │
                            │  - Fact promotion     │
                            │  - Pack execution     │
                            │  - Policy enforcement │
                            └──────────────────────┘
```

### What exists today

- Full HTTP API: truths, timeline, workflow, billing ingress, account summaries
- 4 workbench-executable truth paths running through the current backend:
  `qualify-inbound-lead`, `submit-expense-report`,
  `activate-subscription`, and `refill-prepaid-ai-credits`
- SurrealDB persistence, 109 tests green
- Svelte component library (desktop app — reusable in extension)

### What needs to be built

1. **Chrome extension shell** — manifest v3, side panel, connects to application-server
2. **Job state view** — Svelte component showing active truth execution with
   convergence progress, fact timeline, and blocked steps
3. **Approval action** — button in side panel that triggers approval via API
4. **Live updates** — polling or SSE from server to extension
5. **Demo seed data** — scripted scenario: Acme Corp onboarding with one
   blocked approval step
6. **Narrative deck** — 3-5 slides framing the thesis before the live demo

### What does NOT need to be built

- DOM reading or injection (we're not replicating Whatfix)
- Integration with any specific enterprise app
- Authentication (demo runs local)
- Production deployment

## Positioning for the Meeting

### Don't say

- "We built a better DAP"
- "This replaces Whatfix"
- "CRM" or "ERP"

### Do say

- "We built the job-state layer that knows what work is actually happening"
- "Your guidance becomes intelligent when it knows the state of the job"
- "DAP + Converge = guidance that adapts to reality, not scripts"
- "We govern agent work the same way you guide human work"

### The question to leave them with

> "Would your customers benefit if your walkthroughs knew what job the user
> was doing, what's already been done, and what's actually blocked?"

If yes — partnership conversation.
If no — you learned something about the market.

## Success Criteria

The demo succeeds if:

- [ ] Whatfix sees Converge as complementary, not competitive
- [ ] They understand that job-state awareness is something they lack
- [ ] They ask "how would this integrate with our platform?"
- [ ] Karl walks out with a clear next step (partnership, pilot, or pass)
