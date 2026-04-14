# Blueprint — Monitor Brand Signal (Meltwater replacement)

Status: design blueprint. Not yet in the executable truth catalog.
Scope note: targets Stage 3 (Platform Signal). Do not wire until Stage 1 extension ships.

## 1. What this replaces

Meltwater-class media monitoring / PR intelligence: news, broadcast, blogs,
social, review sites, podcasts. The incumbent value is *ingest everything,
cluster it, alert humans*. The Converge re-framing is: **ingest signal, let
agents propose meaning, converge on a governed brand-state fact, escalate
only when confidence or impact crosses a threshold**.

The job-to-be-done is not "show me mentions". It is:

> "Keep the governed picture of how the brand is being talked about current
>  enough that I can act on real shifts without drowning in noise."

## 2. Domain story

A `BrandWatch` is an intent anchored to a party (org, product, executive,
campaign). It owns a rolling **brand-state** projection: share of voice,
sentiment trajectory, narrative clusters, risk flags, notable sources, and
a list of open `SignalIncidents` awaiting human judgement.

Each ingest cycle:

1. Ports pull raw items from external sources.
2. A normalization agent emits candidate `Mention` proposals.
3. A dedup/cluster agent folds mentions into `NarrativeCluster` proposals.
4. Sentiment, stance, and salience agents enrich clusters.
5. A risk agent proposes `SignalIncident` when a cluster crosses a risk
   threshold (virality, hostile sentiment, executive mention, regulator
   keyword).
6. Converge's promotion gate decides which proposals become facts.
7. The truth executor projects promoted facts into the CRM kernel brand-state
   projection and emits experience events.
8. HITL is required only for `SignalIncident` promotion above a severity
   band, and for any outbound response drafting.

Nothing is auto-published. Humans stay in the loop for anything that leaves
the system.

## 3. Ports (connectors to existing systems)

| Port                    | Direction | Purpose                                       |
|-------------------------|-----------|-----------------------------------------------|
| `port.news.gdelt`       | in        | News + broadcast firehose                     |
| `port.news.rss`         | in        | Curated outlet RSS / Atom                     |
| `port.social.x`         | in        | X/Twitter search + filtered stream            |
| `port.social.reddit`    | in        | Subreddit + search                            |
| `port.social.linkedin`  | in        | Company + exec mentions (where permitted)     |
| `port.podcasts.listen`  | in        | Podcast transcript search                     |
| `port.reviews.g2`       | in        | Review site deltas                            |
| `port.web.crawl`        | in        | Targeted crawl of non-API sources             |
| `port.slack.notify`     | out       | HITL escalation channel                       |
| `port.email.notify`     | out       | Digest + incident alerts                      |
| `port.dam.read`         | in        | Brand assets (for response drafting)          |

Ports are thin. They convert external payloads into a normalized
`RawSignalItem` and nothing else. No judgement in a port.

## 4. Providers (worker instantiations)

| Provider                 | Role                                            |
|--------------------------|-------------------------------------------------|
| `provider.llm.classify`  | Topic, stance, intent classification            |
| `provider.llm.sentiment` | Calibrated sentiment + confidence               |
| `provider.llm.summarize` | Cluster summary + narrative extraction          |
| `provider.embed.text`    | Embeddings for dedup and clustering             |
| `provider.rank.salience` | Salience / virality scoring                     |
| `provider.translate`     | Non-English normalization                       |
| `provider.asr`           | Podcast / broadcast transcription               |

Providers are swappable. A truth binding names *capability*, not vendor.

## 5. Agents (compose Providers + Ports -> proposals)

| Agent                     | Consumes                    | Proposes                           |
|---------------------------|-----------------------------|------------------------------------|
| `normalize-mention`       | ports.in                    | `Mention`                          |
| `dedup-cluster`           | `Mention[]`, embed provider | `NarrativeCluster`, cluster-member |
| `score-sentiment`         | cluster, sentiment provider | `ClusterSentiment`                 |
| `score-salience`          | cluster, rank provider      | `ClusterSalience`                  |
| `extract-narrative`       | cluster, summarize provider | `NarrativeSummary`                 |
| `detect-risk`             | cluster + policy thresholds | `SignalIncident` (HITL-gated)      |
| `project-brand-state`     | promoted facts              | `BrandStateSnapshot` (projection)  |

All agents identify themselves in `Actor::Agent` and emit proposals — never
direct facts. Promotion is Converge's job.

## 6. HITL points

- `SignalIncident` severity >= `high` requires promotion approval.
- Any outbound draft (response post, press statement) is a separate
  `draft-brand-response` truth — not in this one.
- Threshold tuning (what counts as "risk") is an approval-gated policy
  fact, not an agent decision.

## 7. Truth / Root intent sketch

```text
key:            monitor-brand-signal
kind:           Job
pack_ids:       [knowledge, prio-relationship-pack, trust]
runtime:        converge
approval_points:
  - "promote SignalIncident at severity >= high"
  - "revise risk thresholds on the BrandWatch policy"
desired_outcomes:
  - "brand-state projection is current within the watch cadence"
  - "every promoted incident cites traceable source evidence"
  - "no outbound action is taken without human confirmation"
guardrails:
  - "mentions must retain source URL + retrieval timestamp"
  - "clustering must be reproducible from stored embeddings"
  - "sentiment must carry calibrated confidence, not a bare label"
  - "incidents above severity band cannot auto-promote"
modules_touched:
  - parties       (anchor watch to org / exec / product)
  - conversations (store raw items + clusters)
  - documents     (store narrative summaries + incident briefs)
  - memory        (embeddings, semantic retrieval)
  - workflow      (watch cadence, incident lifecycle)
  - approvals     (HITL gate for incidents)
  - intents       (operator-facing BrandWatch context)
```

`TypesRootIntent` shape (informal):

- objective: "maintain a governed brand-signal state for {party}"
- constraints (hard): source attribution, confidence calibration,
  HITL for high-severity
- success criteria (required): projection freshness, incident traceability
- budgets: per-cycle provider token + ingest volume caps

## 8. Gherkin (place at `truths/jobs/monitor_brand_signal.feature`)

```gherkin
Feature: Monitor brand signal
  Scenario: Governed brand-state update from a monitoring cycle
    Given a BrandWatch exists for a party with configured sources
    And external signal ports have new raw items since the last cycle
    When the monitoring truth activates
    Then normalized mentions shall be proposed with source attribution
    And mentions shall be clustered into narrative clusters with reproducible embeddings
    And each cluster shall carry calibrated sentiment and salience
    And the brand-state projection for the party shall be refreshed
    But no outbound response shall be generated by this truth

  Scenario: High-severity incident requires human promotion
    Given a narrative cluster crosses the configured risk threshold
    When the risk agent proposes a SignalIncident at severity high
    Then the incident shall enter the approvals queue
    And the brand-state projection shall show the incident as pending
    But the incident shall not be promoted to a fact without human approval

  Scenario: Weak signal quality must not inflate confidence
    Given fewer than the minimum attributable sources for a cluster
    When sentiment and salience are scored
    Then the cluster shall be marked low-confidence
    And the cluster shall not contribute to share-of-voice aggregates
```

## 9. Open questions

- Do we store raw items in SurrealDB or land them on the Parquet analytical
  path and only project aggregates into the kernel? (Leaning Parquet.)
- Is `BrandWatch` a first-class module or a specialization of `intents`?
- Reuse `knowledge` pack retrieval, or introduce a `signal` pack?
