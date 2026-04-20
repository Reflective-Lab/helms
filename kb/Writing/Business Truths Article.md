---
source: mixed
status: draft
target: blog + linkedin
updated: 2026-04-19
---

# What If Your Business Decisions Could Converge Like Software Tests?

Every organization runs on decisions. Hire this person. Acquire that company. Renew this vendor contract. Launch that pricing change.

Most of these decisions follow a pattern that nobody has formalized:

1. Someone declares what should become true ("we need a go/no-go on this acquisition")
2. Multiple people gather evidence from different angles
3. Contradictions surface ("source A says revenue is $42M, source B says $67M")
4. Someone synthesizes a recommendation
5. Someone with authority approves or blocks it

This pattern repeats across every function — sales, finance, procurement, partnerships, product. The actors change, the evidence sources change, but the structure is identical.

We've been building a system that treats this pattern as a first-class primitive. We call them **Business Truths**.

---

## A Truth is a job to be done, not a task to complete

A Business Truth is a declarative contract: "this is what should become true after this process runs." It's not a checklist. It's not a workflow diagram. It's an outcome specification with explicit success criteria, guardrails, and governance gates.

Here's a concrete example:

```
Truth: Evaluate acquisition target

When: Our board identifies a potential acquisition
Job:   Converge multi-source evidence into a go/no-go recommendation
       with traceable evidence before the LOI deadline

Success criteria:
  - Recommendation produced with confidence >= 0.7
  - All material contradictions surfaced and documented
  - Each dimension (market, tech, financials) cites independent sources

Guardrails:
  - No recommendation without adversarial review
  - Contradictions must be surfaced, never resolved silently
  - Human approval required before recommendation leaves draft
```

Notice what this *doesn't* specify: which databases to query, which APIs to call, which people to involve, or what order to do things in. The Truth declares the outcome. The system figures out how to get there.

## Convergence, not workflow

Traditional business process tools model work as a flowchart: step 1, then step 2, then a decision diamond, then step 3. This breaks down the moment reality gets complicated — when evidence contradicts itself, when a gap requires going back to research, when the team realizes they need expertise they didn't plan for.

We took inspiration from a different domain: convergence in distributed systems. Instead of a linear workflow, we run an adaptive loop:

1. **Seed** initial research strategies (search wide for market context, search deep for financials)
2. **Gather** evidence from multiple sources in parallel
3. **Extract** factual claims with confidence scores and source citations
4. **Detect** contradictions across sources in real time
5. **Identify gaps** and propose follow-up research
6. **Synthesize** when — and only when — the evidence stabilizes

The loop doesn't run for a fixed number of steps. It runs until the facts stop changing or the budget runs out. Both are honest stopping conditions. The system never pretends to know more than it does.

## The governance gate: honest stopping

Here's the part that matters for real organizations: the system can't override human authority.

When the convergence loop detects contradictory claims — say, two credible sources disagree on a company's revenue — it doesn't pick one and move on. It surfaces both claims with their sources, flags the contradiction, and **blocks the recommendation** until a human reviews it.

This isn't a notification. It's a hard gate. The system cannot produce a recommendation while material contradictions remain unreviewed. The human doesn't rubber-stamp; they see the exact claims, the exact sources, and the significance of the disagreement.

This is what we mean by "governed intelligence." The system does the work of gathering, organizing, and synthesizing evidence. But authority stays with humans, and the system is honest about what it knows and doesn't know.

## The formation: right team for the problem

Different decisions need different collaboration structures. A pricing change needs a huddle — cross-functional, collaborative, everyone contributes. An acquisition decision needs a panel — formal review with designated critics, domain experts, and a synthesizer.

The system derives the appropriate team structure from the nature of the decision:

- **Irreversible + high authority** (like an acquisition) → Panel formation with curated experts
- **Reversible + cross-domain** (like a pricing experiment) → Huddle with capability-matched participants
- **Moderate complexity** (like a vendor renewal) → Discussion group with domain leads

This isn't a static mapping. It's learned from outcomes. After running 100 vendor evaluations, the system knows that Panel formations produce better results than Huddles for that problem class. These priors calibrate automatically — the more decisions the organization makes, the better the system gets at structuring the next one.

## Evidence provenance: every fact traces to a source

Every claim in the system carries provenance:

- **Where did this come from?** (which search result, which API, which document)
- **How confident are we?** (0.9 for primary sources like company filings, 0.5 for inferred claims)
- **Does anything contradict it?** (flagged in real time, not post-hoc)
- **Who promoted it?** (which agent extracted it, which human approved it)

When the final recommendation says "the target company has $42M in revenue," you can click through to the exact source that supports that claim. When two sources disagree, both are preserved. The system doesn't hide disagreement; it documents it.

This matters for regulated industries, board-level decisions, and any context where "trust me, the AI said so" isn't good enough.

## The integrity proof: deterministic verification

Every convergence run produces a cryptographic proof: a Merkle root over all facts, a logical clock that ticked on every evidence promotion, and a total fact count. Same inputs, same agents, same run → same Merkle root. You can verify that two runs produced identical governed output without comparing every fact.

This isn't blockchain theater. It's a practical tool for audit: "this recommendation was produced by this exact evidence base, and nothing was added or removed after convergence."

## The learning loop: every decision makes the next one better

After each decision plays out, the system captures a learning episode:

- What did we predict would happen?
- What actually happened?
- Where was the prediction error?
- What should we do differently next time?

These lessons calibrate planning priors — not authority. The system gets better at *planning* decisions (which evidence to gather, which team structure to use, how much research is enough) without getting any say in *making* decisions. Learning flows backward into planning. Authority flows forward through governance. These two streams never cross.

This is the flywheel: more decisions → more episodes → better priors → faster convergence → more decisions worth making this way.

## Five Business Truths we're building

We started with one (acquisition due diligence) and designed four more:

1. **Evaluate acquisition target** — converge multi-source evidence into a go/no-go recommendation
2. **Validate pricing change** — simulate impact across existing customers, revenue, and competitive positioning
3. **Assess vendor renewal** — converge internal usage data with market alternatives and compliance requirements
4. **Qualify partnership opportunity** — determine strategic fit, revenue potential, and integration complexity
5. **Detect churn risk** — converge behavioral signals with competitive context into an intervention plan

Each one follows the same pattern: declare the outcome, let the system figure out evidence gathering and team formation, gate on governance, learn from results.

## What this means for how organizations work

The traditional approach to business decisions is either:

**a) Ad-hoc** — someone does research in a spreadsheet, writes a memo, sends it around for comments, and the boss decides. Evidence is scattered, contradictions are hidden, and institutional learning is zero.

**b) Over-engineered** — a BPM tool encodes a rigid workflow with approval matrices and SLA timers. It works until the process changes, which is constantly. The tool becomes a constraint rather than an enabler.

Business Truths are a third option: **declare the outcome, govern the process, learn from the results.** The structure adapts to the problem. The evidence is traceable. The governance is real. And every decision makes the organization marginally smarter.

We're not replacing human judgment. We're making it possible for humans to make better-informed decisions faster, with full evidence trails, in a system that honestly tells them what it knows and what it doesn't.

That's what governed intelligence means to us.

---

*Karl Pernyer is the founder of Reflective Labs, building Converge — an open platform for governed intelligence.*
