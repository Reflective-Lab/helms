<script lang="ts">
	import { onMount } from 'svelte'

	import {
		analyzeNoteCleanup,
		analyzeNoteValue,
		captureNoteUrl,
		createNote,
		getNoteVaultRoot,
		importAppleNotes,
		importMarkdownTree,
		listNotes,
		moveNote,
		publishAppleNotes,
		readNote,
		saveNote
	} from '$lib/api'
	import { formatTime } from '$lib/format'
	import type {
		AppleNotesImportReport,
		AppleNotesPublishReport,
		NoteCleanupReport,
		NoteValueReport,
		VaultImportReport,
		VaultNote,
		VaultTreeEntry,
		WebSnapshotCaptureReport
	} from '$lib/types'

	let vaultRoot = $state('')
	let entries = $state<VaultTreeEntry[]>([])
	let selectedPath = $state('')
	let currentNote = $state<VaultNote | null>(null)
	let editorBody = $state('')
	let createTitle = $state('')
	let createParentDir = $state('Inbox')
	let moveTargetPath = $state('')
	let captureUrl = $state('')
	let importSourceDir = $state('')
	let importReport = $state<VaultImportReport | null>(null)
	let appleImportReport = $state<AppleNotesImportReport | null>(null)
	let applePublishReport = $state<AppleNotesPublishReport | null>(null)
	let webSnapshotReport = $state<WebSnapshotCaptureReport | null>(null)
	let cleanupReport = $state<NoteCleanupReport | null>(null)
	let valueReport = $state<NoteValueReport | null>(null)
	let loading = $state(true)
	let busy = $state(false)
	let error = $state('')
	let flash = $state('')
	let dirty = $state(false)

	$effect(() => {
		dirty = currentNote ? editorBody !== currentNote.body : false
	})

	async function loadVault(preferredPath?: string) {
		loading = true
		error = ''

		try {
			const [nextVaultRoot, nextEntries] = await Promise.all([getNoteVaultRoot(), listNotes()])
			vaultRoot = nextVaultRoot
			entries = nextEntries

			const notePaths = nextEntries.filter((entry) => entry.kind === 'note').map((entry) => entry.path)
			const nextSelectedPath =
				preferredPath && notePaths.includes(preferredPath)
					? preferredPath
					: selectedPath && notePaths.includes(selectedPath)
						? selectedPath
						: notePaths[0] ?? ''

			if (nextSelectedPath) {
				await openNote(nextSelectedPath)
			} else {
				selectedPath = ''
				currentNote = null
				editorBody = ''
				moveTargetPath = ''
			}
		} catch (cause) {
			error = String(cause)
		} finally {
			loading = false
		}
	}

	async function openNote(path: string) {
		error = ''

		try {
			const note = await readNote(path)
			selectedPath = note.path
			currentNote = note
			editorBody = note.body
			moveTargetPath = note.path
		} catch (cause) {
			error = String(cause)
		}
	}

	async function handleSave() {
		if (!currentNote) return
		busy = true
		error = ''
		flash = ''

		try {
			currentNote = await saveNote(currentNote.path, editorBody)
			editorBody = currentNote.body
			flash = `Saved ${currentNote.path}`
			entries = await listNotes()
		} catch (cause) {
			error = String(cause)
		} finally {
			busy = false
		}
	}

	async function handleCreate() {
		if (!createTitle.trim()) return
		busy = true
		error = ''
		flash = ''

		try {
			const note = await createNote(createTitle.trim(), createParentDir.trim() || undefined)
			createTitle = ''
			flash = `Created ${note.path}`
			await loadVault(note.path)
		} catch (cause) {
			error = String(cause)
		} finally {
			busy = false
		}
	}

	async function handleMove() {
		if (!currentNote || !moveTargetPath.trim()) return
		busy = true
		error = ''
		flash = ''

		try {
			const moved = await moveNote(currentNote.path, moveTargetPath.trim())
			flash = `Moved to ${moved.path}`
			await loadVault(moved.path)
		} catch (cause) {
			error = String(cause)
		} finally {
			busy = false
		}
	}

	async function handleImport() {
		if (!importSourceDir.trim()) return
		busy = true
		error = ''
		flash = ''

		try {
			importReport = await importMarkdownTree(importSourceDir.trim())
			flash = `Imported ${importReport.note_count} notes into ${importReport.imported_root}`
			importSourceDir = ''
			await loadVault()
		} catch (cause) {
			error = String(cause)
		} finally {
			busy = false
		}
	}

	async function handleAppleNotesImport() {
		busy = true
		error = ''
		flash = ''

		try {
			appleImportReport = await importAppleNotes()
			flash = `Imported ${appleImportReport.note_count} Apple Notes into raw run ${appleImportReport.raw_root}`
			await loadVault()
		} catch (cause) {
			error = String(cause)
		} finally {
			busy = false
		}
	}

	async function handleAppleNotesPublish() {
		busy = true
		error = ''
		flash = ''

		try {
			applePublishReport = await publishAppleNotes(appleImportReport?.run_id)
			flash = `Published ${applePublishReport.note_count} Apple Notes into ${applePublishReport.published_root}`
			await loadVault()
		} catch (cause) {
			error = String(cause)
		} finally {
			busy = false
		}
	}

	async function handleCaptureUrl() {
		if (!captureUrl.trim()) return
		busy = true
		error = ''
		flash = ''

		try {
			webSnapshotReport = await captureNoteUrl(captureUrl.trim())
			flash = `Captured ${webSnapshotReport.canonical_url} into ${webSnapshotReport.note_path}`
			captureUrl = ''
			await loadVault(webSnapshotReport.note_path)
		} catch (cause) {
			error = String(cause)
		} finally {
			busy = false
		}
	}

	async function handleCleanupAnalysis() {
		busy = true
		error = ''
		flash = ''

		try {
			cleanupReport = await analyzeNoteCleanup()
			flash = `Analyzed ${cleanupReport.note_count} notes into ${cleanupReport.enriched_root}`
		} catch (cause) {
			error = String(cause)
		} finally {
			busy = false
		}
	}

	async function handleNoteValueAnalysis() {
		busy = true
		error = ''
		flash = ''

		try {
			valueReport = await analyzeNoteValue()
			flash = `Analyzed ${valueReport.note_count} notes for freshness and value into ${valueReport.enriched_root}`
		} catch (cause) {
			error = String(cause)
		} finally {
			busy = false
		}
	}

	onMount(() => {
		void loadVault()
	})
</script>

<div class="page route-page">
	<header class="hero">
		<div>
			<p class="eyebrow">Notes Vault</p>
			<h1>Local-first notes in an Obsidian-compatible vault.</h1>
			<p>
				The desktop app now reads and writes Markdown files directly under <code>~/Notes</code>.
				Standard folders are <code>Inbox</code>, <code>Projects</code>, <code>Areas</code>,
				<code>Resources</code>, and <code>Archive</code>. Hidden pipeline storage like
				<code>.raw</code> stays out of the note tree.
			</p>
		</div>
		<div class="button-row">
			<button class="button secondary" onclick={() => loadVault(selectedPath)} disabled={busy || loading}>
				Refresh Vault
			</button>
			<a class="button secondary button-link" href="/">Back To Workbench</a>
		</div>
	</header>

	{#if error}
		<section class="panel danger">
			<strong>Notes route error</strong>
			<p>{error}</p>
		</section>
	{/if}

	{#if flash}
		<section class="panel flash-panel">
			<strong>Vault update</strong>
			<p>{flash}</p>
		</section>
	{/if}

	{#if loading}
		<section class="panel">
			<p>Loading note vault...</p>
		</section>
	{:else}
		<div class="route-stack">
			<section class="panel">
				<div class="row-between">
					<div>
						<div class="section-title">Vault Root</div>
						<strong>{vaultRoot}</strong>
					</div>
					<div class="meta">
						{entries.filter((entry) => entry.kind === 'note').length} notes ·
						{entries.filter((entry) => entry.kind === 'directory').length} directories
					</div>
				</div>
				{#if importReport}
					<div class="meta">
						Last import: {importReport.note_count} notes and {importReport.attachment_count} attachments into
						{importReport.imported_root}
					</div>
				{/if}
				{#if appleImportReport}
					<div class="meta">
						Apple Notes import: {appleImportReport.note_count} notes, {appleImportReport.attachment_count}
						inline attachments, {appleImportReport.reused_note_count} reused notes,
						{appleImportReport.locked_note_count} locked notes,
						{appleImportReport.timed_out_note_count} timed-out notes into raw run
						{appleImportReport.raw_root}
						.
					</div>
				{/if}
				{#if applePublishReport}
					<div class="meta">
						Apple Notes publish: {applePublishReport.note_count} notes, {applePublishReport.attachment_count}
						attachments, {applePublishReport.created_note_count} created, {applePublishReport.updated_note_count}
						updated into {applePublishReport.published_root}.
					</div>
				{/if}
				{#if webSnapshotReport}
					<div class="meta">
						Web snapshot: {webSnapshotReport.title} into {webSnapshotReport.note_path} with raw run
						{webSnapshotReport.raw_root}.
					</div>
				{/if}
				{#if cleanupReport}
					<div class="meta">
						Cleanup analysis: {cleanupReport.exact_duplicate_group_count} exact duplicate groups,
						{cleanupReport.similarity_candidate_count} similarity candidates, {cleanupReport.merge_suggestion_count}
						merge suggestions in {cleanupReport.enriched_root}.
					</div>
				{/if}
				{#if valueReport}
					<div class="meta">
						Freshness &amp; value: {valueReport.current_note_count} current, {valueReport.aging_note_count}
						aging, {valueReport.stale_note_count} stale. {valueReport.promote_candidate_count} promote,
						{valueReport.refresh_candidate_count} refresh, {valueReport.demote_candidate_count} demote
						candidates in {valueReport.enriched_root}.
					</div>
				{/if}
			</section>

			<div class="notes-layout">
				<aside class="panel notes-sidebar">
					<div class="section-title">Create Note</div>
					<div class="form-grid">
						<label class="field">
							<span>Title</span>
							<input bind:value={createTitle} placeholder="Meeting note" />
						</label>
						<label class="field">
							<span>Parent Directory</span>
							<input bind:value={createParentDir} placeholder="Projects/Converge" />
							<span class="field-help">
								Notes default to <code>Inbox</code>; use structure for lifecycle and tags for facets.
							</span>
						</label>
						<div class="button-row left">
							<button class="button" onclick={handleCreate} disabled={busy || !createTitle.trim()}>
								Create Note
							</button>
						</div>
					</div>

					<div class="section-title notes-section">Cleanup Analysis</div>
					<div class="form-grid">
						<p class="field-help">
							Analyze published notes into <code>~/Notes/.enriched/...</code> for exact duplicates,
							similarity candidates, and merge suggestions.
						</p>
						<div class="button-row left">
							<button class="button secondary" onclick={handleCleanupAnalysis} disabled={busy}>
								Run Cleanup Analysis
							</button>
						</div>
					</div>

					<div class="section-title notes-section">Freshness &amp; Value</div>
					<div class="form-grid">
						<p class="field-help">
							Score visible notes for freshness and current value. This suggests which imported or
							unreviewed notes should be promoted, refreshed, or demoted without rewriting canonical notes.
						</p>
						<div class="button-row left">
							<button class="button secondary" onclick={handleNoteValueAnalysis} disabled={busy}>
								Run Freshness &amp; Value Analysis
							</button>
						</div>
					</div>

					<div class="section-title notes-section">Capture URL</div>
					<div class="form-grid">
						<label class="field">
							<span>Source URL</span>
							<input bind:value={captureUrl} placeholder="https://example.com/post" />
							<span class="field-help">
								The capture writes raw artifacts into <code>~/Notes/.raw/web/...</code> and publishes a note
								stub into <code>Inbox/Web Snapshots</code>.
							</span>
						</label>
						<div class="button-row left">
							<button class="button secondary" onclick={handleCaptureUrl} disabled={busy || !captureUrl.trim()}>
								Capture URL
							</button>
						</div>
					</div>

					<div class="section-title notes-section">Import Markdown Tree</div>
					<div class="form-grid">
						<div class="button-row left">
							<button class="button secondary" onclick={handleAppleNotesImport} disabled={busy}>
								Import Apple Notes
							</button>
							<button class="button secondary" onclick={handleAppleNotesPublish} disabled={busy}>
								Publish Apple Notes
							</button>
						</div>
						<p class="field-help">
							This reads the live Notes app through <code>osascript</code>. macOS may prompt for
							automation access on first run. Locked notes are imported as placeholders.
						</p>
						<p class="field-help">
							Publishing promotes the latest completed raw Apple Notes run into
							<code>~/Notes/Imported/Apple Notes</code> as visible source captures with explicit provenance.
						</p>
						<label class="field">
							<span>Source Directory</span>
							<input bind:value={importSourceDir} placeholder="/Users/you/Exports/Apple Notes" />
							<span class="field-help">
								The importer copies the tree into <code>~/Notes/Imported/...</code>.
							</span>
						</label>
						<div class="button-row left">
							<button class="button secondary" onclick={handleImport} disabled={busy || !importSourceDir.trim()}>
								Import Tree
							</button>
						</div>
					</div>

					<div class="section-title notes-section">Vault Tree</div>
					<div class="note-tree">
						{#if entries.length}
							{#each entries as entry}
								{#if entry.kind === 'directory'}
									<div class="note-tree-label" style={`padding-left: ${entry.depth * 18 + 12}px`}>
										<span class="meta">Folder</span>
										<strong>{entry.name}</strong>
									</div>
								{:else}
									<button
										class:selected={selectedPath === entry.path}
										class="note-tree-button"
										style={`padding-left: ${entry.depth * 18 + 12}px`}
										onclick={() => openNote(entry.path)}
									>
										<strong>{entry.name}</strong>
										<div class="meta">
											{entry.modified_at ? `Updated ${formatTime(entry.modified_at)}` : 'Markdown note'}
										</div>
									</button>
								{/if}
							{/each}
						{:else}
							<p class="empty">The vault is empty. Create a note or import a Markdown tree to start.</p>
						{/if}
					</div>
				</aside>

				<section class="panel notes-detail">
					{#if currentNote}
						<div class="row-between">
							<div>
								<div class="section-title">Editor</div>
								<strong>{currentNote.title}</strong>
								<div class="meta">{currentNote.path}</div>
							</div>
							<div class="meta">
								{#if currentNote.modified_at}
									Last modified {formatTime(currentNote.modified_at)}
								{:else}
									Local Markdown note
								{/if}
								{#if dirty}
									· Unsaved changes
								{/if}
							</div>
						</div>

						<div class="form-grid">
							<label class="field">
								<span>Move Or Rename Path</span>
								<input bind:value={moveTargetPath} placeholder="Projects/Converge/April plan.md" />
							</label>
							<div class="button-row left">
								<button class="button secondary" onclick={handleMove} disabled={busy || !moveTargetPath.trim()}>
									Move Note
								</button>
							</div>
							<label class="field">
								<span>Markdown Body</span>
								<textarea bind:value={editorBody} class="note-editor" spellcheck="false"></textarea>
							</label>
							<div class="button-row left">
								<button class="button" onclick={handleSave} disabled={busy || !dirty}>Save Note</button>
							</div>
						</div>
					{:else}
						<div class="content-section">
							<div class="section-title">Editor</div>
							<p class="empty">Select a note from the vault tree or create a new one.</p>
						</div>
					{/if}

					{#if cleanupReport}
						<div class="content-section">
							<div class="section-title">Cleanup Suggestions</div>
							<div class="meta">
								Report path: <code>{cleanupReport.report_path}</code>
							</div>
							{#if cleanupReport.merge_suggestions.length}
								<div class="form-grid">
									{#each cleanupReport.merge_suggestions.slice(0, 5) as suggestion}
										<div>
											<strong>{suggestion.primary_path}</strong>
											<div class="meta">
												Merge with {suggestion.secondary_path} · {(suggestion.score_bps / 100).toFixed(0)}%
												· {suggestion.rationale}
											</div>
										</div>
									{/each}
								</div>
							{:else}
								<p class="empty">No merge suggestions in the latest cleanup run.</p>
							{/if}
						</div>
					{/if}

					{#if valueReport}
						<div class="content-section">
							<div class="row-between">
								<div>
									<div class="section-title">Freshness &amp; Value Signals</div>
									<div class="meta">Report path: <code>{valueReport.report_path}</code></div>
									<div class="meta">
										Details path: <code>{valueReport.details_path}</code> · Summary path:
										<code>{valueReport.summary_path}</code>
									</div>
								</div>
								<div class="meta">
									{valueReport.current_note_count} current · {valueReport.aging_note_count} aging ·
									{valueReport.stale_note_count} stale
								</div>
							</div>

							<div class="form-grid">
								<div>
									<strong>Promote</strong>
									{#if valueReport.promote_candidates.length}
										{#each valueReport.promote_candidates.slice(0, 5) as candidate}
											<div>
												<strong>{candidate.path}</strong>
												<div class="meta">
													{(candidate.overall_score_bps / 100).toFixed(0)}% overall ·
													{candidate.inbound_reference_count} inbound refs ·
													{candidate.external_url_count} URLs · {candidate.age_days ?? 'unknown'} days old
												</div>
												<div class="meta">{candidate.reasons.join(' · ')}</div>
											</div>
										{/each}
									{:else}
										<p class="empty">No promotion candidates in the latest run.</p>
									{/if}
								</div>

								<div>
									<strong>Refresh</strong>
									{#if valueReport.refresh_candidates.length}
										{#each valueReport.refresh_candidates.slice(0, 5) as candidate}
											<div>
												<strong>{candidate.path}</strong>
												<div class="meta">
													{(candidate.overall_score_bps / 100).toFixed(0)}% overall ·
													{candidate.inbound_reference_count} inbound refs ·
													{candidate.external_url_count} URLs · {candidate.age_days ?? 'unknown'} days old
												</div>
												<div class="meta">{candidate.reasons.join(' · ')}</div>
											</div>
										{/each}
									{:else}
										<p class="empty">No refresh candidates in the latest run.</p>
									{/if}
								</div>

								<div>
									<strong>Demote</strong>
									{#if valueReport.demote_candidates.length}
										{#each valueReport.demote_candidates.slice(0, 5) as candidate}
											<div>
												<strong>{candidate.path}</strong>
												<div class="meta">
													{(candidate.overall_score_bps / 100).toFixed(0)}% overall ·
													{candidate.inbound_reference_count} inbound refs ·
													{candidate.external_url_count} URLs · {candidate.age_days ?? 'unknown'} days old
												</div>
												<div class="meta">{candidate.reasons.join(' · ')}</div>
											</div>
										{/each}
									{:else}
										<p class="empty">No demotion candidates in the latest run.</p>
									{/if}
								</div>
							</div>
						</div>
					{/if}
				</section>
			</div>
		</div>
	{/if}
</div>
