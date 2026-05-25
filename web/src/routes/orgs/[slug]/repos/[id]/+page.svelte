<script lang="ts">
	import { page } from '$app/stores';
	import { onMount, onDestroy } from 'svelte';
	import { api } from '$lib/api';
	import DataTable from '$lib/components/DataTable.svelte';
	import RepoPolicies from '$lib/components/RepoPolicies.svelte';
	import PolicyActivity from '$lib/components/PolicyActivity.svelte';
	import { formatDate } from '$lib/utils/date';

	interface CommitListItem {
		id: string;
		commit_sha: string;
		branch: string | null;
		author: string;
		message: string | null;
		files_changed: number | null;
		ai_sessions_count: number | null;
		committed_at: string | null;
	}

	interface Policy {
		id: string;
		org_id: string;
		repo_id: string | null;
		name: string;
		description: string;
		condition: Record<string, unknown>;
		action: string;
		severity: string;
		scope: string;
		enabled: boolean;
		created_at: string;
		updated_at: string;
	}

	interface Repo {
		id: string;
		name: string;
		github_url: string | null;
		clone_status: string;
		created_at: string;
	}

	let commits: CommitListItem[] = $state([]);
	let commitsTotal = $state(0);
	let policies: Policy[] = $state([]);
	let repo = $state<Repo | null>(null);
	let repoName = $state('');
	let loading = $state(true);
	let policiesLoading = $state(true);
	let policiesError = $state('');
	let error = $state('');
	let syncing = $state(false);
	let expandedCommitId: string | null = $state(null);
	let pollTimer: ReturnType<typeof setInterval> | null = $state(null);

	const repoId = $derived($page.params.id ?? '');
	const slug = $derived($page.params.slug ?? '');
	const cloneStatus = $derived(repo ? repo.clone_status : 'pending');

	onMount(async () => {
		await Promise.all([loadRepo().then(() => loadCommits()), loadPolicies()]);
	});

	onDestroy(() => {
		if (pollTimer) clearInterval(pollTimer);
	});

	async function loadRepo() {
		try {
			const repos = await api.get<Repo[]>(`/api/v1/orgs/${slug}/repos`);
			repo = repos.find((r) => r.id === repoId) ?? null;
			if (repo) repoName = repo.name;
		} catch {
			// non-critical
		}
	}

	async function loadPolicies() {
		policiesLoading = true;
		policiesError = '';
		try {
			policies = (await api.get<Policy[]>(`/api/v1/orgs/${slug}/repos/${repoId}/policies`)) ?? [];
		} catch (err) {
			policiesError = err instanceof Error ? err.message : 'Failed to load policies';
		} finally {
			policiesLoading = false;
		}
	}

	async function loadCommits() {
		try {
			const params = new URLSearchParams({ limit: '200', offset: '0' });
			if (repoId) params.set('repo_id', repoId);
			const result = await api.get<{ items: CommitListItem[]; total: number }>(
				`/api/v1/orgs/${slug}/traces/commits?${params}`
			);
			commits = result?.items ?? [];
			commitsTotal = result?.total ?? 0;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load commits';
		} finally {
			loading = false;
		}
	}

	async function handleSync() {
		syncing = true;
		try {
			const result = await api.post<{ status: string }>(`/api/v1/orgs/${slug}/repos/${repoId}/sync`);
			if (result.status === 'cloning') {
				pollTimer = setInterval(async () => {
					await loadRepo();
					if (repo?.clone_status === 'ready') {
						if (pollTimer) clearInterval(pollTimer);
						pollTimer = null;
						syncing = false;
					}
				}, 3000);
			} else {
				await loadRepo();
				syncing = false;
			}
		} catch (err) {
			error = err instanceof Error ? err.message : 'Sync failed';
			syncing = false;
		}
	}

	function firstLine(msg: string | null): string {
		if (!msg) return '-';
		return msg.split('\n')[0];
	}

	const commitColumns = [
		{ key: 'commit_sha', label: 'Commit', sortable: true },
		{ key: 'message', label: 'Message', sortable: true },
		{ key: 'author', label: 'Author', sortable: true },
		{ key: 'branch', label: 'Branch', sortable: true },
		{ key: 'ai_sessions_count', label: 'AI Sessions', sortable: true },
		{ key: 'files_changed', label: 'Files', sortable: true },
		{ key: 'committed_at', label: 'Date', sortable: true }
	];
</script>

<svelte:head>
	<title>Repo Detail - TraceVault</title>
</svelte:head>

<div class="space-y-6">
	<!-- Header -->
	<div class="flex items-center justify-between">
		<div class="flex items-center gap-2">
			<a href="/orgs/{slug}/repos" class="text-muted-foreground hover:underline">Repos</a>
			<span class="text-muted-foreground">/</span>
			<h1 class="text-2xl font-bold">{repoName || repoId}</h1>
		</div>
		<div class="flex items-center gap-2">
			{#if cloneStatus === 'ready'}
				<a
					href="/orgs/{slug}/repos/{repoId}/code"
					class="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
				>
					<svg class="h-4 w-4" viewBox="0 0 16 16" fill="currentColor">
						<path d="M4.72 3.22a.75.75 0 011.06 1.06L2.06 8l3.72 3.72a.75.75 0 11-1.06 1.06L.47 8.53a.75.75 0 010-1.06l4.25-4.25zm6.56 0a.75.75 0 10-1.06 1.06L13.94 8l-3.72 3.72a.75.75 0 101.06 1.06l4.25-4.25a.75.75 0 000-1.06l-4.25-4.25z" />
					</svg>
					Browse Code
				</a>
			{:else if cloneStatus === 'cloning' || syncing}
				<span class="inline-flex items-center gap-2 rounded-md bg-muted px-4 py-2 text-sm font-medium text-muted-foreground">
					<svg class="h-4 w-4 animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M12 2v4m0 12v4m-7.07-3.93l2.83-2.83m8.48-8.48l2.83-2.83M2 12h4m12 0h4m-3.93 7.07l-2.83-2.83M6.34 6.34L3.51 3.51" />
					</svg>
					Cloning repository...
				</span>
			{:else}
				<button
					onclick={handleSync}
					disabled={syncing || !repo?.github_url}
					class="inline-flex items-center gap-2 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
				>
					<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
					</svg>
					Sync Repository
				</button>
				{#if !repo?.github_url}
					<span class="text-xs text-muted-foreground">No GitHub URL configured</span>
				{:else if cloneStatus === 'error'}
					<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(240,101,101,0.12); color: #f06565; border: 1px solid rgba(240,101,101,0.25)">Clone failed</span>
				{:else}
					<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(79,110,247,0.12); color: #4f6ef7; border: 1px solid rgba(79,110,247,0.25)">Not cloned</span>
				{/if}
			{/if}
			<a
				href="/orgs/{slug}/repos/{repoId}/settings"
				class="inline-flex items-center gap-2 rounded-md border border-input bg-background px-4 py-2 text-sm font-medium hover:bg-accent hover:text-accent-foreground"
			>
				<svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
					<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
					<circle cx="12" cy="12" r="3" />
				</svg>
				Settings
			</a>
		</div>
	</div>

	<!-- Policies -->
	<RepoPolicies
		{slug}
		{repoId}
		{policies}
		loading={policiesLoading}
		error={policiesError}
		onchange={loadPolicies}
	/>

	<!-- Policy Activity -->
	<PolicyActivity {slug} {repoId} {policies} />

	<!-- Commits -->
	<div class="space-y-2">
		<h2 class="text-sm font-semibold">Commits</h2>
		{#if loading}
			<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm">
				<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"></span>
				Loading...
			</div>
		{:else if error}
			<p class="text-destructive">{error}</p>
		{:else if commits.length === 0}
			<p class="text-muted-foreground text-sm">No commits found for this repo.</p>
		{:else}
			{#if commitsTotal > commits.length}
				<p class="text-muted-foreground mb-2 text-xs">
					Showing {commits.length} of {commitsTotal} commits.
					<a href="/orgs/{slug}/traces/commits?repo_id={repoId}" class="text-primary underline-offset-2 hover:underline">View all →</a>
				</p>
			{/if}
			<DataTable
				columns={commitColumns}
				rows={commits}
				searchKeys={['commit_sha', 'message', 'author', 'branch']}
				defaultSort="committed_at"
				defaultSortDir="desc"
				rowIdKey="id"
				onRowClick={(row) => {
					const id = row.id as string;
					expandedCommitId = expandedCommitId === id ? null : id;
				}}
				expandedRowId={expandedCommitId}
			>
				{#snippet children({ row, col })}
					{#if col.key === 'commit_sha'}
						<a
							href="/orgs/{slug}/traces/commits/{row.id}"
							class="font-mono text-sm underline"
							onclick={(e) => e.stopPropagation()}
						>
							{(row.commit_sha as string).slice(0, 8)}
						</a>
					{:else if col.key === 'message'}
						<span class="max-w-xs truncate text-muted-foreground">{firstLine(row.message as string | null)}</span>
					{:else if col.key === 'branch'}
						{#if row.branch}
							<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(167,139,250,0.12); color: #a78bfa; border: 1px solid rgba(167,139,250,0.25)">{row.branch}</span>
						{:else}
							<span class="text-muted-foreground">-</span>
						{/if}
					{:else if col.key === 'ai_sessions_count'}
						{row.ai_sessions_count ?? 0}
					{:else if col.key === 'files_changed'}
						<span class="font-mono">{row.files_changed ?? 0}</span>
					{:else if col.key === 'committed_at'}
						{row.committed_at ? formatDate(row.committed_at as string) : '-'}
					{:else}
						{row[col.key] ?? '-'}
					{/if}
				{/snippet}
				{#snippet expandedRow({ row })}
					{#if row.message}
						<div class="py-3 px-4 bg-muted/20">
							<pre class="whitespace-pre-wrap text-xs text-muted-foreground font-mono">{(row.message as string).trim()}</pre>
						</div>
					{/if}
				{/snippet}
			</DataTable>
		{/if}
	</div>
</div>
