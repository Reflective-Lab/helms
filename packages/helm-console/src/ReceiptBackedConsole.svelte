<script lang="ts">
  import CommandCard from './CommandCard.svelte'
  import EventTimeline from './EventTimeline.svelte'
  import ProofArtifactPanel from './ProofArtifactPanel.svelte'
  import type {
    ConsoleAdapter,
    ConsoleCommandDescriptor,
    ConsoleEvent,
    ProofArtifactSummary,
  } from './types'

  let {
    adapter,
    events = [],
    artifactSummaries = [],
    activePane = 'controls',
    disabled = false,
    onCommand,
    onPaneChange,
  }: {
    adapter: ConsoleAdapter
    events?: ConsoleEvent[]
    artifactSummaries?: ProofArtifactSummary[]
    activePane?: 'controls' | 'events' | 'aids' | 'artifacts'
    disabled?: boolean
    onCommand?: (command: ConsoleCommandDescriptor) => void
    onPaneChange?: (pane: 'controls' | 'events' | 'aids' | 'artifacts') => void
  } = $props()

  const panes = [
    { id: 'controls', label: 'Run' },
    { id: 'events', label: 'Events' },
    { id: 'aids', label: 'Aids' },
    { id: 'artifacts', label: 'Artifacts' },
  ] as const
</script>

<section class="helm-console">
  <header class="helm-console-header">
    <div>
      <p>{adapter.nouns.run}</p>
      <h2>{adapter.displayName}</h2>
    </div>
    <span>{adapter.routePrefix}</span>
  </header>

  <nav aria-label="Console panes">
    {#each panes as pane}
      <button
        type="button"
        class:active={activePane === pane.id}
        onclick={() => onPaneChange?.(pane.id)}
      >
        {pane.label}
      </button>
    {/each}
  </nav>

  {#if activePane === 'controls'}
    <section class="pane">
      {#each adapter.controls as group}
        <article class="group">
          <header>
            <h3>{group.label}</h3>
            <span>{group.commands.length} controls</span>
          </header>
          <div class="cards">
            {#each group.commands as command}
              <CommandCard {command} {disabled} onRun={onCommand} />
            {:else}
              <p class="empty">No mutating controls declared for this group.</p>
            {/each}
          </div>
        </article>
      {/each}
    </section>
  {:else if activePane === 'events'}
    <section class="pane">
      <EventTimeline {events} emptyLabel={`No ${adapter.nouns.event}s loaded.`} />
    </section>
  {:else if activePane === 'aids'}
    <section class="pane aid-grid">
      {#each adapter.aids as aid}
        <article class="aid">
          <p>{aid.aidKind}</p>
          <h3>{aid.label}</h3>
          <span>{aid.authorityBoundary}</span>
          {#if aid.recompute}
            <CommandCard command={aid.recompute} {disabled} onRun={onCommand} />
          {/if}
        </article>
      {:else}
        <p class="empty">No derived aids declared.</p>
      {/each}
    </section>
  {:else}
    <section class="pane">
      <ProofArtifactPanel artifacts={adapter.artifacts} summaries={artifactSummaries} />
    </section>
  {/if}
</section>

<style>
  .helm-console {
    display: grid;
    gap: 0.85rem;
    color: var(--helm-console-ink, #17201d);
  }

  .helm-console-header,
  .group,
  .aid,
  .empty {
    border: 1px solid var(--helm-console-line, #d7dedb);
    border-radius: 0.5rem;
    background: var(--helm-console-surface, #fff);
    padding: 0.85rem;
  }

  .helm-console-header,
  .group > header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  p,
  h2,
  h3 {
    margin: 0;
  }

  .helm-console-header p,
  .helm-console-header span,
  .group > header span,
  .aid p,
  .aid span,
  .empty {
    color: var(--helm-console-muted, #66736e);
    font-size: 0.74rem;
    font-weight: 700;
  }

  h2 {
    font-size: 1.25rem;
  }

  h3 {
    font-size: 1rem;
  }

  nav {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }

  nav button {
    min-height: 2rem;
    border: 1px solid var(--helm-console-line, #d7dedb);
    border-radius: 999px;
    background: #fff;
    color: var(--helm-console-ink, #17201d);
    cursor: pointer;
    font: inherit;
    font-size: 0.82rem;
    font-weight: 700;
    padding: 0.35rem 0.75rem;
  }

  nav button.active {
    border-color: var(--helm-console-action, #0f766e);
    background: var(--helm-console-action, #0f766e);
    color: #fff;
  }

  .pane,
  .cards,
  .group,
  .aid {
    display: grid;
    gap: 0.75rem;
  }

  .aid-grid {
    grid-template-columns: repeat(auto-fit, minmax(18rem, 1fr));
  }

  .aid span {
    line-height: 1.4;
  }
</style>
