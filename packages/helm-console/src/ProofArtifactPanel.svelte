<script lang="ts">
  import type { ConsoleArtifactDescriptor, ProofArtifactSummary } from './types'

  let {
    artifacts = [],
    summaries = [],
  }: {
    artifacts?: ConsoleArtifactDescriptor[]
    summaries?: ProofArtifactSummary[]
  } = $props()

  function summaryFor(artifact: ConsoleArtifactDescriptor): ProofArtifactSummary | undefined {
    return summaries.find((summary) => summary.id === artifact.id)
  }
</script>

<section class="helm-proof-panel" aria-label="Proof artifacts">
  {#each artifacts as artifact}
    {@const summary = summaryFor(artifact)}
    <article>
      <header>
        <div>
          <p>{artifact.resolverScheme ?? 'app://'}</p>
          <h3>{artifact.label}</h3>
        </div>
        <span>{summary?.status ?? 'available'}</span>
      </header>
      {#if summary?.summary}
        <p class="summary">{summary.summary}</p>
      {/if}
      <dl>
        <div>
          <dt>Route</dt>
          <dd>{artifact.read.path}</dd>
        </div>
        <div>
          <dt>Hash</dt>
          <dd>{summary?.contentHash ?? 'pending'}</dd>
        </div>
        <div>
          <dt>Required</dt>
          <dd>{artifact.requiredProvenance?.join(', ') || 'app-defined'}</dd>
        </div>
      </dl>
    </article>
  {:else}
    <p class="empty">No artifacts declared.</p>
  {/each}
</section>

<style>
  .helm-proof-panel {
    display: grid;
    gap: 0.75rem;
  }

  article,
  .empty {
    display: grid;
    gap: 0.65rem;
    border: 1px solid var(--helm-console-line, #d7dedb);
    border-radius: 0.5rem;
    background: var(--helm-console-surface, #fff);
    padding: 0.85rem;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  p,
  h3,
  dl {
    margin: 0;
  }

  header p,
  header span,
  dt,
  .empty {
    color: var(--helm-console-muted, #66736e);
    font-size: 0.72rem;
    font-weight: 700;
  }

  h3 {
    font-size: 1rem;
  }

  .summary {
    color: var(--helm-console-muted, #66736e);
    font-size: 0.86rem;
    line-height: 1.45;
  }

  dl {
    display: grid;
    gap: 0.4rem;
  }

  dl div {
    display: grid;
    grid-template-columns: 5.5rem minmax(0, 1fr);
    gap: 0.5rem;
  }

  dd {
    margin: 0;
    overflow: hidden;
    color: var(--helm-console-ink, #17201d);
    font-size: 0.8rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
