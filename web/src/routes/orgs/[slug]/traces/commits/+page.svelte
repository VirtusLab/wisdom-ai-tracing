<script lang="ts">
	import { page } from '$app/stores';
	import { api } from '$lib/api';
	import { fmtRelativeTime } from '$lib/utils/format';
	import * as Table from '$lib/components/ui/table/index.js';
	import LoadingState from '$lib/components/LoadingState.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import EmptyState from '$lib/components/EmptyState.svelte';
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';

	interface CommitListItem {
		id: string;
		commit_sha: string;
		branch: string | null;
		author: string;
		message: string | null;
		files_changed: number;
		ai_sessions_count: number;
		committed_at: string;
	}

	const slug = $derived($page.params.slug);

	let commits = $state<CommitListItem[]>([]);
	let total = $state(0);
	let loading = $state(true);
	let error = $state('');
	let pageSize = $state(10);
	let currentPage = $state(0);
	let expandedId = $state<string | null>(null);

	function firstLine(msg: string | null): string {
		if (!msg) return '-';
		return msg.split('\n')[0];
	}

	function toggleExpand(id: string) {
		expandedId = expandedId === id ? null : id;
	}

	async function load() {
		loading = true;
		error = '';
		try {
			const params = new URLSearchParams({
				limit: String(pageSize),
				offset: String(currentPage * pageSize)
			});
			const repoId = $page.url.searchParams.get('repo_id');
			const branch = $page.url.searchParams.get('branch');
			const from = $page.url.searchParams.get('from');
			const to = $page.url.searchParams.get('to');
			if (repoId) params.set('repo_id', repoId);
			if (branch) params.set('branch', branch);
			if (from) params.set('from', from);
			if (to) params.set('to', to);

			const result = await api.get<{ items: CommitListItem[]; total: number }>(
				`/api/v1/orgs/${slug}/traces/commits?${params}`
			);
			commits = result?.items ?? [];
			total = result?.total ?? 0;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load commits';
		} finally {
			loading = false;
		}
	}

	function setPageSize(s: number) {
		pageSize = s;
		currentPage = 0;
		load();
	}

	let search = $state('');

	const filteredCommits = $derived(
		search.trim()
			? commits.filter((c) => {
					const q = search.toLowerCase();
					return (
						c.commit_sha?.toLowerCase().includes(q) ||
						c.author?.toLowerCase().includes(q) ||
						c.message?.toLowerCase().includes(q) ||
						c.branch?.toLowerCase().includes(q)
					);
				})
			: commits
	);

	const totalPages = $derived(Math.max(1, Math.ceil(total / pageSize)));
	const showFrom = $derived(total === 0 ? 0 : currentPage * pageSize + 1);
	const showTo = $derived(Math.min((currentPage + 1) * pageSize, total));

	$effect(() => { slug; $page.url.searchParams; load(); });
</script>

<svelte:head>
	<title>Commits - TraceVault</title>
</svelte:head>

<div class="space-y-4">
	{#if loading}
		<LoadingState />
	{:else if error}
		<ErrorState message={error} onRetry={load} />
	{:else if commits.length === 0}
		<EmptyState message="No commits yet." />
	{:else}
		<div class="border-border overflow-hidden rounded-lg border">
			<!-- Search bar -->
			<div class="border-border flex items-center gap-2 border-b px-3 py-2">
				<SearchIcon class="text-muted-foreground h-3.5 w-3.5 shrink-0" />
				<input
					type="text"
					placeholder="Search SHA, author, message, branch (this page)…"
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
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Commit</Table.Head>
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Message</Table.Head>
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Branch</Table.Head>
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Author</Table.Head>
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Files</Table.Head>
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">AI Sessions</Table.Head>
						<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Committed</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each filteredCommits as c (c.id)}
						<Table.Row
							class="hover:bg-muted/40 cursor-pointer transition-colors"
							onclick={() => toggleExpand(c.id)}
						>
							<Table.Cell>
								<a
									href="/orgs/{slug}/traces/commits/{c.id}"
									class="font-mono text-sm underline"
									onclick={(e) => e.stopPropagation()}
								>
									{c.commit_sha.slice(0, 8)}
								</a>
							</Table.Cell>
							<Table.Cell class="text-muted-foreground max-w-xs truncate">{firstLine(c.message)}</Table.Cell>
							<Table.Cell>
								{#if c.branch}
									<span class="rounded-full px-2 py-0.5 text-[10px]"
										style="background: rgba(79,110,247,0.12); color: #4f6ef7; border: 1px solid rgba(79,110,247,0.25)"
									>{c.branch}</span>
								{:else}
									<span class="text-muted-foreground">-</span>
								{/if}
							</Table.Cell>
							<Table.Cell class="text-muted-foreground">{c.author}</Table.Cell>
							<Table.Cell class="font-mono">{c.files_changed}</Table.Cell>
							<Table.Cell class="font-mono">{c.ai_sessions_count}</Table.Cell>
							<Table.Cell class="text-muted-foreground">{fmtRelativeTime(c.committed_at)}</Table.Cell>
						</Table.Row>
						{#if expandedId === c.id && c.message}
							<Table.Row>
								<Table.Cell colspan={7} class="p-0">
									<div class="bg-muted/20 px-4 py-3">
										<pre class="text-muted-foreground whitespace-pre-wrap font-mono text-xs">{c.message.trim()}</pre>
									</div>
								</Table.Cell>
							</Table.Row>
						{/if}
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
