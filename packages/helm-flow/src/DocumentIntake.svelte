<script lang="ts">
  /**
   * Generic document intake and readiness display.
   *
   * Reusable across any decision type:
   * - Vendor selection RFI/RFP packages
   * - Due diligence research collections
   * - Any workflow requiring document prerequisites
   *
   * Props are minimal; domain layer provides content.
   */

  interface Document {
    name: string
    kind: string
    size: string
  }

  interface ExpectedDoc {
    title: string
    purpose: string
    info: string
  }

  interface Props {
    documents?: Document[]
    fastLoadEnabled?: boolean
    executableReady?: boolean
    expectedDocs?: ExpectedDoc[]
    onFilesSelected?: (files: File[]) => void
  }

  let {
    documents = $bindable([]),
    fastLoadEnabled = $bindable(false),
    executableReady = $bindable(false),
    expectedDocs,
    onFilesSelected = () => {},
  }: Props = $props()

  let dialogOpen = $state(false)
  let fileInput: HTMLInputElement | undefined = $state()

  function handleFiles(fileList: FileList | null) {
    if (!fileList) return
    const files = Array.from(fileList).slice(0, 6)
    onFilesSelected(files)
  }

  function handleDrop(event: DragEvent) {
    event.preventDefault()
    handleFiles(event.dataTransfer?.files ?? null)
  }

  function handleFileInput(event: Event) {
    handleFiles((event.currentTarget as HTMLInputElement).files)
  }

  function closeDialog() {
    dialogOpen = false
  }

  function openDialog() {
    dialogOpen = true
  }

  function triggerFileSelect() {
    fileInput?.click()
  }

  let documentReady = $derived(documents.length >= 3)
  let readinessLabel = $derived(documentReady ? 'Ready' : `${documents.length}/3`)
  let readinessIcon = $derived(documentReady ? 'ok' : 'warn')
</script>

<div
  class="rounded-2xl border border-dashed border-border bg-raised p-5 transition hover:border-subtle"
  ondrop={handleDrop}
  ondragover={(e) => e.preventDefault()}
>
  <div class="flex items-start justify-between gap-4">
    <label class="block flex-1 cursor-pointer">
      <input
        type="file"
        bind:this={fileInput}
        class="hidden"
        multiple
        onchange={handleFileInput}
      />
      <span class="block text-xs font-semibold uppercase tracking-widest text-muted">Decision Package</span>
      <h2 class="mt-1 font-display text-xl font-semibold text-bright">Drop your documents</h2>
      <p class="mt-1 text-sm text-subtle">3+ supporting documents for the decision.</p>
      <p class="mt-3 font-mono text-xs text-muted">Click to browse or drag files here.</p>
    </label>
    {#if expectedDocs}
      <button type="button" class="btn-ghost text-xs" onclick={openDialog}>
        What's needed?
      </button>
    {/if}
  </div>

  <div class="mt-5 grid gap-3 md:grid-cols-2">
    <button
      type="button"
      class="flex items-center justify-between gap-3 rounded-xl border px-3 py-2 transition"
      class:border-lime={fastLoadEnabled}
      class:bg-lime-glow={fastLoadEnabled}
      class:border-border={!fastLoadEnabled}
      class:bg-deep={!fastLoadEnabled}
      onclick={() => (fastLoadEnabled = !fastLoadEnabled)}
    >
      <span>
        <span class="block text-sm text-text">Fast Load</span>
        <span class="block text-xs text-muted">Use sample package</span>
      </span>
      <span class="h-2.5 w-2.5 rounded-full" class:bg-ok={fastLoadEnabled} class:bg-muted={!fastLoadEnabled}></span>
    </button>

    <label class="flex items-start gap-3 rounded-xl border border-border bg-deep px-3 py-2 transition cursor-pointer">
      <input class="mt-1 accent-lime" type="checkbox" bind:checked={executableReady} />
      <span>
        <span class="block text-sm text-text">Executable + Converging Truth</span>
        <span class="block text-xs text-muted">Ready to execute</span>
      </span>
    </label>
  </div>

  {#if documents.length > 0}
    <div class="mt-4 space-y-2">
      {#each documents as doc}
        <div class="flex items-center justify-between gap-3 rounded-xl border border-border bg-raised px-3 py-2">
          <div class="min-w-0">
            <p class="truncate text-sm text-bright">{doc.name}</p>
            <p class="text-xs text-muted">{doc.kind}</p>
          </div>
          <span class="font-mono text-xs text-subtle">{doc.size}</span>
        </div>
      {/each}
    </div>
  {:else}
    <p class="mt-4 text-sm text-subtle">No documents yet.</p>
  {/if}
</div>

<div class="mt-3 rounded-xl border border-border bg-deep px-3 py-2">
  <div class="flex items-center gap-2">
    <span class="h-2.5 w-2.5 rounded-full" class:bg-ok={documentReady} class:bg-warn={!documentReady}></span>
    <span class="font-display text-sm font-semibold text-bright">Package Ready</span>
  </div>
  <p class="mt-1 text-xs text-muted">{readinessLabel}</p>
</div>

{#if dialogOpen && expectedDocs}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-void/84 px-4 py-8 backdrop-blur-sm"
    role="presentation"
    onclick={closeDialog}
    onkeydown={(event) => event.key === 'Escape' && closeDialog()}
  >
    <div
      class="max-h-[90vh] w-full max-w-2xl overflow-auto rounded-2xl border border-border bg-deep p-6 shadow-2xl"
      role="dialog"
      aria-modal="true"
      aria-labelledby="expected-docs-title"
      onclick={(event) => event.stopPropagation()}
      onkeydown={(event) => event.stopPropagation()}
    >
      <div class="mb-5 flex items-start justify-between gap-4">
        <div>
          <span class="card-label">Expected Documents</span>
          <h2 id="expected-docs-title" class="mt-1 font-display text-2xl font-semibold text-bright">
            What documents are needed?
          </h2>
          <p class="mt-2 text-sm text-subtle">
            The system requires 3+ documents, but complete runs use all available categories.
          </p>
        </div>
        <button class="btn-ghost text-sm" type="button" onclick={closeDialog}>
          Close
        </button>
      </div>

      <div class="grid gap-3 md:grid-cols-2">
        {#each expectedDocs as doc}
          <article class="rounded-xl border border-border bg-raised p-4">
            <h3 class="font-display text-base font-semibold text-bright">{doc.title}</h3>
            <p class="mt-2 text-sm text-subtle">{doc.purpose}</p>
            <p class="mt-3 text-xs text-muted">{doc.info}</p>
          </article>
        {/each}
      </div>
    </div>
  </div>
{/if}

