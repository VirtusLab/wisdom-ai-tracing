<script lang="ts">
	import { page } from '$app/stores';
	import { api } from '$lib/api';
	import { useFetch } from '$lib/hooks/use-fetch.svelte';
	import { fmtNum, fmtCost, fmtRelativeTime } from '$lib/utils/format';
	import { sessionStatus } from '$lib/utils/status';
	import { getToolColor } from '$lib/utils/colors';
	import type {
		SessionMetadataResponse,
		EventItem,
		FileChange,
		TranscriptResponse,
		LinkedCommit
	} from '$lib/types';
	import { formatDateTime } from '$lib/utils/date';
	import * as Table from '$lib/components/ui/table/index.js';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import LoadingState from '$lib/components/LoadingState.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import SessionTranscript from '$lib/components/session-detail/SessionTranscript.svelte';

	let expandedEvents = $state(new Set<string>());
	let expandedFiles = $state(new Set<string>());
	let sectionsOpen = $state({
		events: false,
		files: false,
		transcript: false,
		commits: false
	});

	// Per-panel lazy-loaded data (null = not yet fetched, acts as cache)
	let eventsData = $state<EventItem[] | null>(null);
	let eventsLoading = $state(false);
	let eventsError = $state<string | null>(null);

	let fileChangesData = $state<FileChange[] | null>(null);
	let fileChangesLoading = $state(false);
	let fileChangesError = $state<string | null>(null);

	let transcriptData = $state<TranscriptResponse | null>(null);
	let transcriptLoading = $state(false);
	let transcriptError = $state<string | null>(null);

	let linkedCommitsData = $state<LinkedCommit[] | null>(null);
	let linkedCommitsLoading = $state(false);
	let linkedCommitsError = $state<string | null>(null);

	const slug = $derived($page.params.slug);
	const sessionId = $derived($page.params.id);

	const detailQuery = useFetch<SessionMetadataResponse>(
		() => `/api/v1/orgs/${slug}/traces/sessions/${sessionId}`
	);

	// Secondary fetch for cost breakdown (from analytics detail endpoint)
	interface CostBreakdown {
		input_cost: number;
		output_cost: number;
		cache_read_cost: number;
		cache_write_cost: number;
		total_cost: number;
	}
	interface AnalyticsDetail {
		cost_breakdown: CostBreakdown;
	}

	const analyticsDetailQuery = useFetch<AnalyticsDetail>(
		() => `/api/v1/orgs/${slug}/analytics/sessions/${sessionId}/detail`
	);

	const costBreakdown = $derived(analyticsDetailQuery.data?.cost_breakdown ?? null);

	const baseUrl = $derived(`/api/v1/orgs/${slug}/traces/sessions/${sessionId}`);

	async function fetchPanelData(key: keyof typeof sectionsOpen) {
		if (key === 'events' && eventsData === null) {
			eventsLoading = true;
			eventsError = null;
			try {
				eventsData = await api.get<EventItem[]>(`${baseUrl}/events`);
			} catch (err: unknown) {
				eventsError = err instanceof Error ? err.message : String(err);
			} finally {
				eventsLoading = false;
			}
		} else if (key === 'files' && fileChangesData === null) {
			fileChangesLoading = true;
			fileChangesError = null;
			try {
				fileChangesData = await api.get<FileChange[]>(`${baseUrl}/file-changes`);
			} catch (err: unknown) {
				fileChangesError = err instanceof Error ? err.message : String(err);
			} finally {
				fileChangesLoading = false;
			}
		} else if (key === 'transcript' && transcriptData === null) {
			transcriptLoading = true;
			transcriptError = null;
			try {
				transcriptData = await api.get<TranscriptResponse>(`${baseUrl}/transcript`);
			} catch (err: unknown) {
				transcriptError = err instanceof Error ? err.message : String(err);
			} finally {
				transcriptLoading = false;
			}
		} else if (key === 'commits' && linkedCommitsData === null) {
			linkedCommitsLoading = true;
			linkedCommitsError = null;
			try {
				linkedCommitsData = await api.get<LinkedCommit[]>(`${baseUrl}/linked-commits`);
			} catch (err: unknown) {
				linkedCommitsError = err instanceof Error ? err.message : String(err);
			} finally {
				linkedCommitsLoading = false;
			}
		}
	}

	function eventSummary(event: EventItem): string {
		if (!event.tool_input) return '';
		const input = event.tool_input as Record<string, unknown>;
		if (input.file_path) return String(input.file_path);
		if (input.path) return String(input.path);
		if (input.command) {
			const cmd = String(input.command);
			return cmd.length > 80 ? cmd.slice(0, 80) + '...' : cmd;
		}
		if (input.pattern) return String(input.pattern);
		return '';
	}

	function formatJson(obj: unknown): string {
		try {
			return JSON.stringify(obj, null, 2);
		} catch {
			return String(obj);
		}
	}

	function toggleEvent(id: string) {
		const next = new Set(expandedEvents);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		expandedEvents = next;
	}

	function toggleSection(key: keyof typeof sectionsOpen) {
		const opening = !sectionsOpen[key];
		sectionsOpen = { ...sectionsOpen, [key]: opening };
		if (opening) {
			fetchPanelData(key);
		}
	}

	function toggleFile(id: string) {
		const next = new Set(expandedFiles);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		expandedFiles = next;
	}

	interface DiffLine {
		type: 'add' | 'remove' | 'header';
		content: string;
	}

	function parseDiff(diffText: string): DiffLine[] {
		const lines: DiffLine[] = [];
		for (const raw of diffText.split('\n')) {
			if (raw.startsWith('+++') || raw.startsWith('---')) {
				lines.push({ type: 'header', content: raw });
			} else if (raw.startsWith('+')) {
				lines.push({ type: 'add', content: raw });
			} else if (raw.startsWith('-')) {
				lines.push({ type: 'remove', content: raw });
			} else {
				// context line or empty
				lines.push({ type: 'header', content: raw });
			}
		}
		return lines;
	}

</script>

<svelte:head>
	<title>Session Detail - TraceVault</title>
</svelte:head>

<div class="space-y-5">
	{#if detailQuery.loading}
		<LoadingState />
	{:else if detailQuery.error}
		<ErrorState message={detailQuery.error} onRetry={detailQuery.refetch} />
	{:else if detailQuery.data}
		{@const data = detailQuery.data}
		{@const session = data.session}
		{@const status = sessionStatus(session.status, session.updated_at)}

		<!-- Breadcrumb + header -->
		<div class="flex items-center gap-3">
			<a
				href="/orgs/{slug}/traces/sessions"
				class="text-muted-foreground text-sm transition-colors hover:text-foreground"
			>
				Traces
			</a>
			<span class="text-muted-foreground/40">/</span>
			<a
				href="/orgs/{slug}/traces/sessions"
				class="text-muted-foreground text-sm transition-colors hover:text-foreground"
			>
				Sessions
			</a>
			<span class="text-muted-foreground/40">/</span>
			<span class="font-mono text-sm font-semibold">{session.session_id.slice(0, 8)}</span>
			<StatusBadge {status} />
		</div>

		<!-- Session metadata -->
		<div class="border-border overflow-hidden rounded-lg border">
			<div class="bg-muted/30 flex flex-wrap items-center gap-x-6 gap-y-1 px-4 py-3 text-sm">
				<div class="flex items-center gap-2">
					<span class="text-muted-foreground text-xs uppercase tracking-wide">Session ID</span>
					<span class="font-mono text-xs">{session.session_id}</span>
				</div>
				<div class="flex items-center gap-2">
					<span class="text-muted-foreground text-xs uppercase tracking-wide">Repo</span>
					<span class="text-xs">{session.repo_name}</span>
				</div>
				{#if session.user_email}
					<div class="flex items-center gap-2">
						<span class="text-muted-foreground text-xs uppercase tracking-wide">User</span>
						<span class="text-xs">{session.user_email}</span>
					</div>
				{/if}
				{#if session.cwd}
					<div class="flex items-center gap-2">
						<span class="text-muted-foreground text-xs uppercase tracking-wide">CWD</span>
						<span class="font-mono text-xs">{session.cwd}</span>
					</div>
				{/if}
				<div class="flex items-center gap-2">
					<span class="text-muted-foreground text-xs uppercase tracking-wide">Started</span>
					<span class="text-xs">{formatDateTime(session.started_at)}</span>
				</div>
			</div>

			<!-- Stats row -->
			<div class="grid grid-cols-2 gap-px md:grid-cols-5">
				<div class="bg-background p-3">
					<div class="text-muted-foreground text-[11px] uppercase tracking-wide">Events</div>
					<div class="mt-1 text-lg font-semibold">{fmtNum(data.counts.events)}</div>
				</div>
				<div class="bg-background p-3">
					<div class="text-muted-foreground text-[11px] uppercase tracking-wide">Files</div>
					<div class="mt-1 text-lg font-semibold">{fmtNum(data.counts.file_changes)}</div>
				</div>
				<div class="bg-background p-3">
					<div class="text-muted-foreground text-[11px] uppercase tracking-wide">Tokens</div>
					<div class="mt-1 text-lg font-semibold">{fmtNum(((session.input_tokens ?? 0) + (session.output_tokens ?? 0) + (session.cache_read_tokens ?? 0) + (session.cache_write_tokens ?? 0)) || session.total_tokens)}</div>
					{#if (session.output_tokens ?? 0) > 0 || (session.cache_read_tokens ?? 0) > 0}
						<div class="text-muted-foreground mt-1 space-y-0.5 text-[10px] leading-tight">
							<div><span class="inline-block w-6">in:</span>{fmtNum(session.input_tokens)}</div>
							<div><span class="inline-block w-6">out:</span>{fmtNum(session.output_tokens)}</div>
							<div><span class="inline-block w-6">cr:</span>{fmtNum(session.cache_read_tokens)}</div>
							<div><span class="inline-block w-6">cw:</span>{fmtNum(session.cache_write_tokens)}</div>
						</div>
					{/if}
				</div>
				<div class="bg-background p-3">
					<div class="text-muted-foreground text-[11px] uppercase tracking-wide">Cost</div>
					<div class="mt-1 text-lg font-semibold">{fmtCost(session.estimated_cost_usd)}</div>
					{#if costBreakdown && (costBreakdown.output_cost > 0 || costBreakdown.cache_read_cost > 0 || costBreakdown.input_cost > 0)}
						<div class="text-muted-foreground mt-1 space-y-0.5 text-[10px] leading-tight">
							<div><span class="inline-block w-6">in:</span>{fmtCost(costBreakdown.input_cost)}</div>
							<div><span class="inline-block w-6">out:</span>{fmtCost(costBreakdown.output_cost)}</div>
							<div><span class="inline-block w-6">cr:</span>{fmtCost(costBreakdown.cache_read_cost)}</div>
							<div><span class="inline-block w-6">cw:</span>{fmtCost(costBreakdown.cache_write_cost)}</div>
						</div>
					{/if}
				</div>
				<div class="bg-background p-3">
					<div class="text-muted-foreground text-[11px] uppercase tracking-wide">Commits</div>
					<div class="mt-1 text-lg font-semibold">{fmtNum(data.counts.linked_commits)}</div>
				</div>
			</div>
		</div>

		<!-- Accordion sections -->
		<div class="space-y-3">
			<!-- File Changes -->
			<div class="border-border overflow-hidden rounded-lg border">
				<button
					class="hover:bg-muted/40 flex w-full items-center gap-3 px-4 py-3 text-left transition-colors"
					onclick={() => toggleSection('files')}
				>
					<span class="text-muted-foreground/50 text-xs">{sectionsOpen.files ? '▼' : '▶'}</span>
					<span class="text-sm font-semibold">File Changes</span>
					<span class="text-muted-foreground ml-auto text-xs">{data.counts.file_changes} files</span>
				</button>
				{#if sectionsOpen.files}
					<div class="border-border border-t">
						{#if fileChangesLoading}
							<div class="px-4 py-4"><LoadingState /></div>
						{:else if fileChangesError}
							<ErrorState message={fileChangesError} onRetry={() => { fileChangesData = null; fetchPanelData('files'); }} />
						{:else if fileChangesData}
							{#if fileChangesData.length === 0}
								<p class="text-muted-foreground px-4 py-4 text-sm">No file changes recorded.</p>
							{:else}
								<div class="divide-border divide-y">
									{#each fileChangesData as fc (fc.id)}
										<div>
											<button
												class="hover:bg-muted/30 flex w-full items-center gap-2 px-4 py-2.5 text-left text-xs transition-colors"
												onclick={() => toggleFile(fc.id)}
											>
												<span class="text-muted-foreground/50 text-xs shrink-0">{expandedFiles.has(fc.id) ? '▼' : '▶'}</span>
												<span
													class="inline-flex shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium
														{fc.change_type === 'create'
															? 'bg-green-500/15 text-green-600 dark:text-green-400'
															: 'bg-amber-500/15 text-amber-600 dark:text-amber-400'}"
												>
													{fc.change_type}
												</span>
												<span class="font-mono font-medium">{fc.file_path.split('/').pop()}</span>
												<span class="text-muted-foreground truncate font-mono">{fc.file_path}</span>
											</button>
											{#if expandedFiles.has(fc.id)}
												<div class="border-border border-t bg-muted/30">
													{#if fc.diff_text}
														{@const diffLines = parseDiff(fc.diff_text)}
														<div class="overflow-x-auto">
															<table class="w-full border-collapse">
																<tbody>
																	{#each diffLines as line, i}
																		<tr class="{line.type === 'add'
																			? 'bg-green-100 dark:bg-green-500/15'
																			: line.type === 'remove'
																				? 'bg-red-100 dark:bg-red-500/15'
																				: ''}">
																			<td class="select-none border-r border-border/40 px-2 text-right font-mono text-[10px] leading-relaxed {line.type === 'remove' ? 'text-red-400/60 dark:text-red-400/50' : line.type === 'add' ? 'text-green-500/60 dark:text-green-400/50' : 'text-muted-foreground/40'}" style="width: 1px; white-space: nowrap;">
																				{i + 1}
																			</td>
																			<td class="select-none border-r border-border/40 px-1.5 text-center font-mono text-[11px] leading-relaxed {line.type === 'add' ? 'text-green-600 dark:text-green-400' : line.type === 'remove' ? 'text-red-600 dark:text-red-400' : 'text-muted-foreground/40'}" style="width: 1px;">
																				{line.type === 'add' ? '+' : line.type === 'remove' ? '-' : ' '}
																			</td>
																			<td class="px-3 font-mono text-[11px] leading-relaxed whitespace-pre {line.type === 'add'
																				? 'text-green-800 dark:text-green-300'
																				: line.type === 'remove'
																					? 'text-red-800 dark:text-red-300'
																					: 'text-foreground'}">
																				{line.type === 'add' ? line.content.slice(1) : line.type === 'remove' ? line.content.slice(1) : line.type === 'header' && (line.content.startsWith('+++') || line.content.startsWith('---')) ? line.content : line.content}
																			</td>
																		</tr>
																	{/each}
																</tbody>
															</table>
														</div>
													{:else}
														<div class="px-4 py-3 text-xs text-muted-foreground italic">
															{fc.change_type === 'create' ? 'New file created (no diff available)' : 'No diff data available'}
														</div>
													{/if}
												</div>
											{/if}
										</div>
									{/each}
								</div>
							{/if}
						{/if}
					</div>
				{/if}
			</div>

			<!-- Transcript -->
			<div class="border-border overflow-hidden rounded-lg border">
				<button
					class="hover:bg-muted/40 flex w-full items-center gap-3 px-4 py-3 text-left transition-colors"
					onclick={() => toggleSection('transcript')}
				>
					<span class="text-muted-foreground/50 text-xs">{sectionsOpen.transcript ? '▼' : '▶'}</span>
					<span class="text-sm font-semibold">Transcript</span>
					<span class="text-muted-foreground ml-auto text-xs">{data.counts.transcript_records} records</span>
				</button>
				{#if sectionsOpen.transcript}
					<div class="border-border border-t px-4 py-4">
						{#if transcriptLoading}
							<LoadingState />
						{:else if transcriptError}
							<ErrorState message={transcriptError} onRetry={() => { transcriptData = null; fetchPanelData('transcript'); }} />
						{:else if transcriptData}
							{#if transcriptData.transcript_records.length === 0}
								<p class="text-muted-foreground text-sm">No transcript data.</p>
							{:else}
								<SessionTranscript records={transcriptData.transcript_records} />
							{/if}
						{/if}
					</div>
				{/if}
			</div>

			<!-- Linked Commits -->
			<div class="border-border overflow-hidden rounded-lg border">
				<button
					class="hover:bg-muted/40 flex w-full items-center gap-3 px-4 py-3 text-left transition-colors"
					onclick={() => toggleSection('commits')}
				>
					<span class="text-muted-foreground/50 text-xs">{sectionsOpen.commits ? '▼' : '▶'}</span>
					<span class="text-sm font-semibold">Linked Commits</span>
					<span class="text-muted-foreground ml-auto text-xs">{data.counts.linked_commits} commits</span>
				</button>
				{#if sectionsOpen.commits}
					<div class="border-border border-t">
						{#if linkedCommitsLoading}
							<div class="px-4 py-4"><LoadingState /></div>
						{:else if linkedCommitsError}
							<ErrorState message={linkedCommitsError} onRetry={() => { linkedCommitsData = null; fetchPanelData('commits'); }} />
						{:else if linkedCommitsData}
							{#if linkedCommitsData.length === 0}
								<p class="text-muted-foreground px-4 py-4 text-sm">No linked commits.</p>
							{:else}
								<Table.Root class="text-xs">
									<Table.Header>
										<Table.Row>
											<Table.Head>Commit SHA</Table.Head>
											<Table.Head>Branch</Table.Head>
											<Table.Head>Confidence</Table.Head>
										</Table.Row>
									</Table.Header>
									<Table.Body>
										{#each linkedCommitsData as commit (commit.commit_id)}
											<Table.Row class="hover:bg-muted/40 transition-colors">
												<Table.Cell>
													<a
														href="/orgs/{slug}/traces/commits/{commit.commit_id}"
														class="font-mono text-sm underline"
													>
														{commit.commit_sha.slice(0, 8)}
													</a>
												</Table.Cell>
												<Table.Cell>
													{#if commit.branch}
														<span
															class="inline-flex rounded-full border-transparent bg-blue-100 px-2 py-0.5 text-[10px] font-medium text-blue-700 dark:bg-blue-950 dark:text-blue-300"
														>
															{commit.branch}
														</span>
													{:else}
														<span class="text-muted-foreground">-</span>
													{/if}
												</Table.Cell>
												<Table.Cell class="font-mono text-sm">
													{#if commit.confidence != null}
														{(commit.confidence * 100).toFixed(0)}%
													{:else}
														-
													{/if}
												</Table.Cell>
											</Table.Row>
										{/each}
									</Table.Body>
								</Table.Root>
							{/if}
						{/if}
					</div>
				{/if}
			</div>

			<!-- Events Timeline -->
			<div class="border-border overflow-hidden rounded-lg border">
				<button
					class="hover:bg-muted/40 flex w-full items-center gap-3 px-4 py-3 text-left transition-colors"
					onclick={() => toggleSection('events')}
				>
					<span class="text-muted-foreground/50 text-xs">{sectionsOpen.events ? '▼' : '▶'}</span>
					<span class="text-sm font-semibold">Events Timeline</span>
					<span class="text-muted-foreground ml-auto text-xs">{data.counts.events} events</span>
				</button>
				{#if sectionsOpen.events}
					<div class="border-border border-t">
						{#if eventsLoading}
							<div class="px-4 py-4"><LoadingState /></div>
						{:else if eventsError}
							<ErrorState message={eventsError} onRetry={() => { eventsData = null; fetchPanelData('events'); }} />
						{:else if eventsData}
							{#if eventsData.length === 0}
								<p class="text-muted-foreground px-4 py-4 text-sm">No events recorded.</p>
							{:else}
								<div class="divide-border divide-y">
									{#each eventsData as event (event.id)}
										{@const color = getToolColor(event.tool_name)}
										<div>
											<button
												class="hover:bg-muted/30 flex w-full items-center gap-3 px-4 py-2 text-left text-xs transition-colors"
												onclick={() => toggleEvent(event.id)}
											>
												<span class="h-2.5 w-2.5 shrink-0 rounded-full {color}"></span>
												<span class="w-16 shrink-0 font-mono font-medium">{event.tool_name ?? event.event_type}</span>
												<span class="text-muted-foreground min-w-0 flex-1 truncate font-mono">{eventSummary(event)}</span>
												<span class="text-muted-foreground shrink-0">{fmtRelativeTime(event.timestamp)}</span>
											</button>
											{#if expandedEvents.has(event.id)}
												<div class="border-border border-t px-4 py-3">
													{#if event.tool_input}
														<div class="mb-2">
															<span class="text-muted-foreground text-[10px] uppercase tracking-wide">Input</span>
															<pre class="bg-muted/20 mt-1 max-h-60 overflow-auto rounded p-3 font-mono text-[11px] leading-relaxed">{formatJson(event.tool_input)}</pre>
														</div>
													{/if}
													{#if event.tool_response}
														<div>
															<span class="text-muted-foreground text-[10px] uppercase tracking-wide">Response</span>
															<pre class="bg-muted/20 mt-1 max-h-60 overflow-auto rounded p-3 font-mono text-[11px] leading-relaxed">{formatJson(event.tool_response)}</pre>
														</div>
													{/if}
												</div>
											{/if}
										</div>
									{/each}
								</div>
							{/if}
						{/if}
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>
