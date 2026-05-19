<script lang="ts">
  /**
   * Generic HITL (Human In The Loop) approval form.
   *
   * No domain assumptions about what the decision is.
   * Domain layer binds decision package content.
   * Converge/Axiom decide why the gate exists; Helm just renders the form.
   */

  interface Props {
    decisionSummary?: {
      candidate?: string
      reason?: string
      threshold?: string
    }
    approverName?: string
    approvalNote?: string
    delegateToPolicy?: boolean
    policyPreview?: string
    onApprove?: () => void
    onReject?: () => void
    disabled?: boolean
  }

  let {
    decisionSummary = {},
    approverName = $bindable(''),
    approvalNote = $bindable(''),
    delegateToPolicy = $bindable(false),
    policyPreview = '',
    onApprove = () => {},
    onReject = () => {},
    disabled = false,
  }: Props = $props()

  function handleApprove(event: SubmitEvent) {
    event.preventDefault()
    onApprove()
  }

  function handleReject() {
    onReject()
  }
</script>

<form class="rounded-2xl border border-warn/30 bg-warn/5 p-4" onsubmit={handleApprove}>
  <span class="block text-xs font-semibold uppercase tracking-widest text-muted">HITL Gate</span>
  <h2 class="mt-1 font-display text-xl font-semibold text-bright">Human approval required.</h2>
  <p class="mt-2 text-sm text-subtle">The governed process requires human review before promoting this decision.</p>

  {#if decisionSummary.candidate || decisionSummary.reason}
    <div class="mt-4 grid gap-3 md:grid-cols-2">
      {#if decisionSummary.candidate}
        <div class="rounded-xl border border-border bg-deep p-3">
          <span class="card-label">Recommendation</span>
          <p class="mt-1 text-sm text-bright">{decisionSummary.candidate}</p>
        </div>
      {/if}
      {#if decisionSummary.reason}
        <div class="rounded-xl border border-border bg-deep p-3">
          <span class="card-label">Gate Reason</span>
          <p class="mt-1 text-sm text-bright">{decisionSummary.reason}</p>
        </div>
      {/if}
      {#if decisionSummary.threshold}
        <div class="rounded-xl border border-border bg-deep p-3">
          <span class="card-label">Threshold</span>
          <p class="mt-1 text-sm text-bright">{decisionSummary.threshold}</p>
        </div>
      {/if}
    </div>
  {/if}

  <div class="mt-4 grid gap-3">
    <label class="block">
      <span class="card-label mb-1 block">Approver</span>
      <input
        class="w-full rounded-xl border border-border bg-deep px-3 py-2 text-sm text-text focus:border-lime/50 focus:outline-none"
        type="email"
        bind:value={approverName}
        {disabled}
        placeholder="approver@example.com"
      />
    </label>
    <label class="block">
      <span class="card-label mb-1 block">Decision Note</span>
      <textarea
        class="min-h-20 w-full rounded-xl border border-border bg-deep px-3 py-2 text-sm text-text focus:border-lime/50 focus:outline-none"
        bind:value={approvalNote}
        {disabled}
        placeholder="Rationale for approval..."
      ></textarea>
    </label>
    <label class="flex items-start gap-3 rounded-xl border border-lime/20 bg-lime-glow p-3">
      <input class="mt-1 accent-lime" type="checkbox" bind:checked={delegateToPolicy} {disabled} />
      <span>
        <strong class="block text-sm text-bright">Delegate to policy for matching future cases</strong>
        <span class="text-xs text-subtle">Auto-approve next time when these conditions recur.</span>
      </span>
    </label>
  </div>

  {#if policyPreview}
    <pre class="mt-4 overflow-auto rounded-xl border border-border bg-deep p-3 font-mono text-xs leading-relaxed text-subtle">{policyPreview}</pre>
  {/if}

  <div class="mt-4 flex flex-col gap-2 sm:flex-row">
    <button class="btn-lime flex-1 justify-center" type="submit" {disabled}>
      Approve And Promote
    </button>
    <button class="flex-1 rounded-xl border border-warn/40 px-4 py-2 text-sm font-semibold text-warn transition hover:bg-warn/10 disabled:cursor-not-allowed disabled:opacity-60" type="button" onclick={handleReject} {disabled}>
      Reject
    </button>
  </div>
</form>
