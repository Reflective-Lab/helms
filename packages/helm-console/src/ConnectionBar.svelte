<script lang="ts">
  let {
    appName,
    baseUrl,
    bearerToken = '',
    status = 'idle',
    onBaseUrlChange,
    onBearerTokenChange,
    onRefresh,
  }: {
    appName: string
    baseUrl: string
    bearerToken?: string
    status?: string
    onBaseUrlChange?: (value: string) => void
    onBearerTokenChange?: (value: string) => void
    onRefresh?: () => void
  } = $props()
</script>

<section class="helm-connection-bar" aria-label="{appName} connection">
  <strong>{appName}</strong>
  <label>
    <span>API</span>
    <input value={baseUrl} oninput={(event) => onBaseUrlChange?.(event.currentTarget.value)} />
  </label>
  <label>
    <span>Bearer</span>
    <input
      type="password"
      autocomplete="off"
      value={bearerToken}
      oninput={(event) => onBearerTokenChange?.(event.currentTarget.value)}
    />
  </label>
  <button type="button" onclick={() => onRefresh?.()}>Refresh</button>
  <span class="status">{status}</span>
</section>

<style>
  .helm-connection-bar {
    display: grid;
    grid-template-columns: auto minmax(10rem, 1fr) minmax(10rem, 0.7fr) auto auto;
    align-items: end;
    gap: 0.65rem;
    border: 1px solid var(--helm-console-line, #d7dedb);
    border-radius: 0.5rem;
    background: var(--helm-console-surface, #fff);
    padding: 0.75rem;
  }

  strong {
    align-self: center;
    white-space: nowrap;
  }

  label {
    display: grid;
    gap: 0.25rem;
  }

  label span,
  .status {
    color: var(--helm-console-muted, #66736e);
    font-size: 0.72rem;
    font-weight: 700;
  }

  input {
    min-height: 2rem;
    min-width: 0;
    border: 1px solid var(--helm-console-line, #d7dedb);
    border-radius: 0.4rem;
    padding: 0.35rem 0.5rem;
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

  .status {
    align-self: center;
    border: 1px solid var(--helm-console-line, #d7dedb);
    border-radius: 999px;
    padding: 0.35rem 0.6rem;
  }

  @media (max-width: 760px) {
    .helm-connection-bar {
      grid-template-columns: 1fr;
      align-items: stretch;
    }
  }
</style>
