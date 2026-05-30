<script lang="ts">
  import type { ConsoleEvent } from './types'

  let {
    events = [],
    emptyLabel = 'No events yet.',
  }: {
    events?: ConsoleEvent[]
    emptyLabel?: string
  } = $props()

  function eventType(event: ConsoleEvent): string {
    return event.type || 'event'
  }

  function eventPosition(event: ConsoleEvent): string {
    if (event.sequence !== undefined) return `#${event.sequence}`
    if (event.event_id) return event.event_id
    return 'unsequenced'
  }
</script>

<section class="helm-event-timeline" aria-label="Event timeline">
  {#each events as event}
    <article>
      <div>
        <strong>{eventType(event)}</strong>
        <span>{eventPosition(event)}</span>
      </div>
      {#if event.occurred_at}
        <time>{event.occurred_at}</time>
      {/if}
    </article>
  {:else}
    <p>{emptyLabel}</p>
  {/each}
</section>

<style>
  .helm-event-timeline {
    display: grid;
    gap: 0.5rem;
  }

  article,
  p {
    border: 1px solid var(--helm-console-line, #d7dedb);
    border-radius: 0.5rem;
    background: var(--helm-console-surface, #fff);
    padding: 0.7rem;
  }

  article div {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  strong {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  span,
  time,
  p {
    margin: 0;
    color: var(--helm-console-muted, #66736e);
    font-size: 0.78rem;
  }
</style>
