<script lang="ts">
	import { page } from '$app/stores';
	import { api } from '$lib/api';
	import DataTable from '$lib/components/DataTable.svelte';
	import LoadingState from '$lib/components/LoadingState.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';

	interface BranchItem {
		branch: string;
		repo_name: string;
		tag: string | null;
		commits_count: number;
		sessions_count: number;
		total_cost: number | null;
		status: string;
		last_activity: string | null;
	}

	let branches: BranchItem[] = $state([]);
	let loading = $state(true);
	let error = $state('');

	const slug = $derived($page.params.slug);

	async function fetchData() {
		loading = true;
		error = '';
		try {
			const repoId = $page.url.searchParams.get('repo_id');
			const qs = repoId ? `?repo_id=${repoId}` : '';
			branches = await api.get<BranchItem[]>(`/api/v1/orgs/${slug}/traces/branches${qs}`);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load branches';
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		fetchData();
	});

	function fmtCost(n: number | null): string {
		if (n == null) return '-';
		return `$${n.toFixed(2)}`;
	}

	function fmtRelativeTime(iso: string | null): string {
		if (!iso) return '-';
		const diff = Date.now() - new Date(iso).getTime();
		const minutes = Math.floor(diff / 60000);
		const hours = Math.floor(minutes / 60);
		const days = Math.floor(hours / 24);
		if (days > 0) return `${days}d ago`;
		if (hours > 0) return `${hours}h ago`;
		if (minutes > 0) return `${minutes}m ago`;
		return 'just now';
	}

	const STATUS_STYLES: Record<string, { bg: string; color: string; border: string }> = {
		tracked: {
			bg: 'rgba(59,130,246,0.12)',
			color: '#3b82f6',
			border: 'rgba(59,130,246,0.25)'
		},
		merged: {
			bg: 'rgba(34,197,94,0.12)',
			color: '#22c55e',
			border: 'rgba(34,197,94,0.25)'
		},
		tagged: {
			bg: 'rgba(168,85,247,0.12)',
			color: '#a855f6',
			border: 'rgba(168,85,247,0.25)'
		}
	};

	function statusStyle(status: string): { bg: string; color: string; border: string } {
		return (
			STATUS_STYLES[status] ?? {
				bg: 'rgba(148,163,184,0.12)',
				color: '#94a3b8',
				border: 'rgba(148,163,184,0.25)'
			}
		);
	}

	const columns = [
		{ key: 'repo_name', label: 'Repo', sortable: true },
		{ key: 'branch', label: 'Branch', sortable: true },
		{ key: 'commits_count', label: 'Commits', sortable: true },
		{ key: 'sessions_count', label: 'Sessions', sortable: true },
		{ key: 'total_cost', label: 'Cost', sortable: true },
		{ key: 'status', label: 'Status', sortable: true },
		{ key: 'last_activity', label: 'Last Activity', sortable: true }
	];
</script>

<svelte:head>
	<title>Branches - TraceVault</title>
</svelte:head>

<div class="space-y-4">
	<h1 class="text-2xl font-bold">Branches</h1>

	{#if loading}
		<LoadingState />
	{:else if error}
		<ErrorState message={error} onRetry={fetchData} />
	{:else if branches.length === 0}
		<p class="text-muted-foreground">No branches tracked yet.</p>
	{:else}
		<DataTable
			{columns}
			rows={branches}
			searchKeys={['branch', 'status']}
			defaultSort="last_activity"
			defaultSortDir="desc"
			rowIdKey="branch"
		>
			{#snippet children({ row, col })}
				{#if col.key === 'branch'}
					<span
						class="rounded-full px-2 py-0.5 text-[10px]"
						style="background: rgba(79,110,247,0.12); color: #4f6ef7; border: 1px solid rgba(79,110,247,0.25)"
						>{row.branch}</span
					>
				{:else if col.key === 'commits_count'}
					<span class="font-mono text-sm">{row.commits_count}</span>
				{:else if col.key === 'sessions_count'}
					<span class="font-mono text-sm">{row.sessions_count}</span>
				{:else if col.key === 'total_cost'}
					<span class="font-mono text-sm">{fmtCost(row.total_cost as number | null)}</span>
				{:else if col.key === 'status'}
					{@const ss = statusStyle(row.status as string)}
					<span
						class="rounded-full px-2 py-0.5 text-[10px]"
						style="background: {ss.bg}; color: {ss.color}; border: 1px solid {ss.border}"
						>{row.status}</span
					>
				{:else if col.key === 'last_activity'}
					<span class="text-muted-foreground">{fmtRelativeTime(row.last_activity as string | null)}</span>
				{:else}
					{row[col.key] ?? '-'}
				{/if}
			{/snippet}
		</DataTable>
	{/if}
</div>
