<script lang="ts">
  import type { ConsoleCommandDescriptor } from './types'

  let {
    command,
    disabled = false,
    onRun,
  }: {
    command: ConsoleCommandDescriptor
    disabled?: boolean
    onRun?: (command: ConsoleCommandDescriptor) => void
  } = $props()

  const authorityLabel = $derived(command.authority.replace('-', ' '))
</script>

<article class="helm-command-card" data-authority={command.authority}>
  <header>
    <div>
      <p>{authorityLabel}</p>
      <h3>{command.label}</h3>
    </div>
    <button type="button" disabled={disabled} onclick={() => onRun?.(command)}>Run</button>
  </header>

  {#if command.description}
    <p class="description">{command.description}</p>
  {/if}

  <dl>
    <div>
      <dt>Method</dt>
      <dd>{command.request.method}</dd>
    </div>
    <div>
      <dt>Route</dt>
      <dd>{command.request.path}</dd>
    </div>
    <div>
      <dt>Events</dt>
      <dd>{command.expectedEventTypes?.join(', ') || 'declared by app'}</dd>
    </div>
  </dl>
</article>

<style>
  .helm-command-card {
    display: grid;
    gap: 0.75rem;
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
  dt {
    color: var(--helm-console-muted, #66736e);
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
  }

  h3 {
    font-size: 0.98rem;
  }

  button {
    min-height: 2rem;
    border: 1px solid var(--helm-console-action, #0f766e);
    border-radius: 0.4rem;
    background: var(--helm-console-action, #0f766e);
    color: #fff;
    cursor: pointer;
    font: inherit;
    font-size: 0.82rem;
    font-weight: 700;
    padding: 0.35rem 0.65rem;
  }

  button:disabled {
    border-color: var(--helm-console-line, #d7dedb);
    background: #d8dfdc;
    cursor: not-allowed;
  }

  .description {
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
    grid-template-columns: 5rem minmax(0, 1fr);
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
