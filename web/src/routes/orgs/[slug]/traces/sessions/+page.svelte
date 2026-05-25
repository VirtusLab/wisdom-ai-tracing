<script lang="ts">
	import { page } from '$app/stores';
	import { api } from '$lib/api';
	import { fmtNum, fmtRelativeTime } from '$lib/utils/format';
	import { sessionStatus } from '$lib/utils/status';
	import type { SessionItem } from '$lib/types';
	import * as Table from '$lib/components/ui/table/index.js';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import LoadingState from '$lib/components/LoadingState.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import EmptyState from '$lib/components/EmptyState.svelte';
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';

	type StatusFilter = 'all' | 'active' | 'completed' | 'stale';

	const slug = $derived($page.params.slug);

	let statusFilter = $state<StatusFilter>('all');
	let pageSize = $state(10);
	let currentPage = $state(0);

	let sessions = $state<SessionItem[]>([]);
	let total = $state(0);
	let loading = $state(true);
	let error = $state('');

	async function load() {
		loading = true;
		error = '';
		try {
			const params = new URLSearchParams({
				limit: String(pageSize),
				offset: String(currentPage * pageSize)
			});
			const repoId = $page.url.searchParams.get('repo_id');
			const from = $page.url.searchParams.get('from');
			const to = $page.url.searchParams.get('to');
			if (repoId) params.set('repo_id', repoId);
			if (from) params.set('from', from);
			if (to) params.set('to', to);
			if (statusFilter !== 'all') params.set('status', statusFilter);

			const result = await api.get<{ items: SessionItem[]; total: number }>(
				`/api/v1/orgs/${slug}/traces/sessions?${params}`
			);
			sessions = result?.items ?? [];
			total = result?.total ?? 0;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load sessions';
		} finally {
			loading = false;
		}
	}

	function setFilter(f: StatusFilter) {
		statusFilter = f;
		currentPage = 0;
		load();
	}

	function setPageSize(s: number) {
		pageSize = s;
		currentPage = 0;
		load();
	}

	const filterButtons: { value: StatusFilter; label: string }[] = [
		{ value: 'all', label: 'All' },
		{ value: 'active', label: 'Active' },
		{ value: 'completed', label: 'Completed' },
		{ value: 'stale', label: 'Stale' }
	];

	let search = $state('');

	const filteredSessions = $derived(
		search.trim()
			? sessions.filter((s) => {
					const q = search.toLowerCase();
					return (
						s.session_id?.toLowerCase().includes(q) ||
						s.repo_name?.toLowerCase().includes(q)
					);
				})
			: sessions
	);

	const totalPages = $derived(Math.max(1, Math.ceil(total / pageSize)));
	const showFrom = $derived(total === 0 ? 0 : currentPage * pageSize + 1);
	const showTo = $derived(Math.min((currentPage + 1) * pageSize, total));

	$effect(() => {
		slug; statusFilter; $page.url.searchParams;
		load();
	});
</script>

<svelte:head>
	<title>Sessions - TraceVault</title>
</svelte:head>

<div class="space-y-4">
	<!-- Status filter -->
	<div class="flex gap-1">
		{#each filterButtons as btn}
			<button
				class="rounded-md px-3 py-1.5 text-xs font-medium transition-colors
					{statusFilter === btn.value
						? 'bg-primary text-primary-foreground'
						: 'bg-muted text-muted-foreground hover:text-foreground'}"
				onclick={() => setFilter(btn.value)}
			>
				{btn.label}
			</button>
		{/each}
	</div>

	{#if loading}
		<LoadingState />
	{:else if error}
		<ErrorState message={error} onRetry={load} />
	{:else if sessions.length === 0}
		<EmptyState message="No sessions found." />
	{:else}
		<div class="border-border overflow-hidden rounded-lg border">
			<!-- Search bar -->
			<div class="border-border flex items-center gap-2 border-b px-3 py-2">
				<SearchIcon class="text-muted-foreground h-3.5 w-3.5 shrink-0" />
				<input
					type="text"
					placeholder="Search session ID or repo (this page)…"
					bind:value={search}
					class="text-foreground placeholder:text-muted-foreground w-full bg-transparent text-sm outline-none"
				/>
				{#if search}
					<button class="text-muted-foreground hover:text-foreground" onclick={() => (search = '')}>
						<XIcon class="h-3.5 w-3.5" />
					</button>
				{/if}
			</div>
			<Table.Root class="text-xs">
				<Table.Header>
					<Table.Row class="bg-muted/30 border-border border-b">
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Status</Table.Head>
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Session ID</Table.Head>
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Repo</Table.Head>
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Tool Calls</Table.Head>
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Tokens (total)</Table.Head>
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Started</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each filteredSessions as s (s.id)}
						<Table.Row class="hover:bg-muted/40 transition-colors">
							<Table.Cell><StatusBadge status={sessionStatus(s.status, s.updated_at)} /></Table.Cell>
							<Table.Cell>
								<a href="/orgs/{slug}/traces/sessions/{s.id}" class="font-mono text-sm underline">
									{String(s.session_id).slice(0, 8)}
								</a>
							</Table.Cell>
							<Table.Cell>{s.repo_name ?? '-'}</Table.Cell>
							<Table.Cell class="font-mono">{fmtNum(s.total_tool_calls)}</Table.Cell>
							<Table.Cell>
								{@const actualTotal = (s.input_tokens ?? 0) + (s.output_tokens ?? 0) + (s.cache_read_tokens ?? 0) + (s.cache_write_tokens ?? 0)}
								<div class="font-mono text-xs">{fmtNum(actualTotal || s.total_tokens)}</div>
								{#if (s.input_tokens ?? 0) > 0 || (s.output_tokens ?? 0) > 0 || (s.cache_read_tokens ?? 0) > 0 || (s.cache_write_tokens ?? 0) > 0}
									<div class="text-muted-foreground mt-0.5 text-[10px] leading-tight">
										<span>in:{fmtNum(s.input_tokens)}</span>
										<span class="ml-1">out:{fmtNum(s.output_tokens)}</span>
										<br />
										<span>cr:{fmtNum(s.cache_read_tokens)}</span>
										<span class="ml-1">cw:{fmtNum(s.cache_write_tokens)}</span>
									</div>
								{/if}
							</Table.Cell>
							<Table.Cell class="text-muted-foreground">{fmtRelativeTime(s.started_at)}</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>

			<!-- Pagination footer -->
			<div class="border-border text-muted-foreground flex items-center justify-between border-t px-3 py-2 text-xs">
				<span>{showFrom}-{showTo} of {total}</span>
				<div class="flex items-center gap-3">
					<span>Per page:</span>
					{#each [10, 25, 50] as size}
						<button
							class="rounded px-1.5 py-0.5 transition-colors {pageSize === size
								? 'bg-primary text-primary-foreground'
								: 'hover:text-foreground'}"
							onclick={() => setPageSize(size)}
						>
							{size}
						</button>
					{/each}
					<span class="text-border mx-1">|</span>
					<button
						class="hover:text-foreground disabled:opacity-30"
						disabled={currentPage === 0}
						onclick={() => { currentPage--; load(); }}
					>
						<ChevronLeftIcon class="h-4 w-4" />
					</button>
					<span>{currentPage + 1}/{totalPages}</span>
					<button
						class="hover:text-foreground disabled:opacity-30"
						disabled={currentPage >= totalPages - 1}
						onclick={() => { currentPage++; load(); }}
					>
						<ChevronRightIcon class="h-4 w-4" />
					</button>
				</div>
			</div>
		</div>
	{/if}
</div>
