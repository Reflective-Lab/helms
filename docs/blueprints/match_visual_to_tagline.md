# Blueprint — Match Visual to Tagline (internal marketing, at scale)

Status: design blueprint. Not yet in the executable truth catalog.
Scope note: targets Stage 3. Do not wire until Stage 1 extension ships.

## 1. The job

> "Given a campaign brief and a library of brand-safe assets, produce
>  governed visual + tagline pairings at scale — with the brand manager,
>  marketer, and storyteller each seeing their part, and a human signing
>  before anything leaves the system."

This is not "generate an image". It is a multi-actor, brand-governed
matching and approval motion over an existing asset library plus generated
candidates, where the final picked pair is a **promoted fact** tied to a
campaign.

## 2. Actors

| Actor role          | Responsibility                                              |
|---------------------|-------------------------------------------------------------|
| `campaign-owner`    | Issues the brief, owns outcome                              |
| `brand-manager`     | Guardrails: brand voice, palette, tone, prohibited themes   |
| `marketer`          | Audience fit, channel constraints, performance priors       |
| `storyteller`       | Narrative coherence, tagline craft, emotional arc           |
| `legal-reviewer`    | IP / claims / compliance sign-off (conditional)             |
| `runtime-agent`     | Agents running inside the truth                             |

Each human role has its own HITL surface and its own approval scope. A
brand manager never approves copy craft; a storyteller never approves
legal claims. Approvals are typed.

## 3. Domain story

A `CampaignBrief` carries audience, channels, goals, hard constraints
(forbidden claims, regulated markets), and a deadline. The truth:

1. Retrieves brand guardrails + prior performance priors from memory.
2. Retrieves candidate visuals from the DAM via port.
3. Optionally proposes *generated* visual candidates via an image provider,
   tagged as synthetic and never auto-promoted.
4. Drafts tagline candidates with a storyteller-style provider.
5. Scores every `(visual, tagline)` pair for: brand-fit, audience-fit,
   narrative coherence, novelty, risk.
6. Produces a ranked `PairingSlate` proposal.
7. Routes the slate through typed HITL: brand -> marketing -> story ->
   (legal if flagged).
8. On full approval, promotes the chosen `CampaignPairing` fact and
   projects it into campaign state. Nothing ships from this truth; a
   downstream publish truth is separate.

## 4. Ports

| Port                    | Direction | Purpose                                   |
|-------------------------|-----------|-------------------------------------------|
| `port.dam.read`         | in        | Read brand-safe asset library             |
| `port.dam.write`        | out       | Write back approved pairing metadata      |
| `port.figma.read`       | in        | Read layout frames / templates            |
| `port.analytics.priors` | in        | Historical performance by cohort/channel  |
| `port.slack.review`     | out       | Per-role review prompts                   |
| `port.email.review`     | out       | Fallback review channel                   |
| `port.legal.intake`     | out       | Open a legal review ticket when flagged   |

## 5. Providers

| Provider                   | Role                                              |
|----------------------------|---------------------------------------------------|
| `provider.llm.copy`        | Tagline generation in storyteller voice           |
| `provider.llm.critique`    | Brand + narrative coherence critique              |
| `provider.vision.caption`  | Describe visuals in text                          |
| `provider.vision.safety`   | Brand-safety and content-risk checks on visuals   |
| `provider.embed.multimodal`| Joint image+text embedding for pairing scoring    |
| `provider.image.generate`  | Synthetic visual candidates (optional, flagged)   |
| `provider.rank.priors`     | Score against historical performance priors      |

## 6. Agents

| Agent                    | Consumes                                 | Proposes                       |
|--------------------------|------------------------------------------|--------------------------------|
| `retrieve-assets`        | brief, dam port, multimodal embed        | `VisualCandidate[]`            |
| `draft-taglines`         | brief, copy provider, brand guardrails   | `TaglineCandidate[]`           |
| `generate-visuals`       | brief, image provider (if enabled)       | `VisualCandidate[]` (synthetic)|
| `score-brand-fit`        | candidates, critique, brand policy       | `BrandFitScore`                |
| `score-audience-fit`     | candidates, priors provider              | `AudienceFitScore`             |
| `score-narrative`        | pairs, critique                          | `NarrativeScore`               |
| `screen-risk`            | visual safety + claims check             | `RiskFlag`                     |
| `rank-pairings`          | all scores                               | `PairingSlate` (ranked)        |
| `route-review`           | slate, actor roles                       | role-typed approval requests   |
| `project-pairing`        | fully-approved slate                     | `CampaignPairing` (promoted)   |

## 7. HITL — typed approvals

Approvals are not a single button. The slate is decomposed:

- **Brand approval**: palette, voice, prohibited-theme check.
- **Marketing approval**: channel + audience + priors trade-off.
- **Story approval**: tagline craft + narrative coherence.
- **Legal approval**: only if `screen-risk` raised a flag.

Converge records each approval as a typed fact with the approving actor.
A pairing cannot be promoted until all *required* roles have approved. A
missing legal flag does not require legal approval; a raised flag does.

This is the "different actors + HITL" shape you asked for: the flow is
multi-gate, not single-gate.

## 8. Truth / Root intent sketch

```text
key:            match-visual-to-tagline
kind:           Job
pack_ids:       [knowledge, prio-relationship-pack, prio-work-pack, trust]
runtime:        converge
approval_points:
  - "brand-manager approval of brand fit"
  - "marketer approval of audience fit"
  - "storyteller approval of narrative + copy"
  - "legal approval when a risk flag is raised"
desired_outcomes:
  - "a governed CampaignPairing fact exists for the brief"
  - "every chosen pairing cites brand, audience, and narrative evidence"
  - "synthetic visuals are never promoted without explicit human approval"
guardrails:
  - "taglines must comply with brand voice policy facts"
  - "synthetic visuals must be flagged and never auto-selected"
  - "risk-flagged pairings require legal approval"
  - "priors used in scoring must be traceable to an analytics source"
modules_touched:
  - intents     (CampaignBrief and desired outcome)
  - documents   (briefs, pairings, approvals, rationale)
  - memory      (brand guardrails, priors, embeddings)
  - workflow    (multi-gate approval lifecycle)
  - approvals   (typed approval gates per role)
  - parties     (campaign owner, approvers)
```

## 9. Gherkin (place at `truths/jobs/match_visual_to_tagline.feature`)

```gherkin
Feature: Match visual to tagline
  Scenario: Governed pairing slate from a campaign brief
    Given a CampaignBrief with audience, channels, and constraints
    And a brand-safe asset library is reachable via the DAM port
    And brand guardrail facts exist for the party
    When the match truth activates
    Then visual candidates shall be retrieved with provenance
    And tagline candidates shall be drafted in the storyteller voice
    And every (visual, tagline) pair shall carry brand, audience, and narrative scores
    And a ranked PairingSlate shall be proposed
    But no pairing shall be promoted without the required human approvals

  Scenario: Multi-role approval gates a single pairing
    Given a ranked PairingSlate has been proposed
    When the top pairing is routed for review
    Then the brand-manager shall approve brand fit
    And the marketer shall approve audience fit
    And the storyteller shall approve narrative and copy
    And only after all required approvals shall the CampaignPairing fact be promoted

  Scenario: Risk flag triggers legal approval
    Given the risk screen raises a claims or IP flag on a pairing
    When that pairing is routed for review
    Then a legal review request shall be opened via the legal intake port
    And the pairing shall not be promoted without a recorded legal approval fact

  Scenario: Synthetic visual candidates are never auto-selected
    Given the image generation provider is enabled for this brief
    When synthetic visuals are proposed alongside DAM visuals
    Then every synthetic candidate shall be tagged as synthetic
    And a synthetic candidate shall require explicit human selection before promotion
```

## 10. Open questions

- Do priors live in `memory` or in a new `analytics` module surface?
- Is `CampaignPairing` a first-class module or a specialization of
  `documents` + `workflow`?
- Should typed approvals (brand / marketing / story / legal) live as
  sub-kinds under `approvals`, or as separate approval packs?
- Where does the synthetic-visual policy live — brand guardrails, or a
  dedicated `synthetic-media` policy truth?
