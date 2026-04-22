<script lang="ts">
	import { page } from '$app/stores';
	import { onMount, onDestroy } from 'svelte';
	import { api } from '$lib/api';
	import * as Table from '$lib/components/ui/table/index.js';
	import DataTable from '$lib/components/DataTable.svelte';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import * as Select from '$lib/components/ui/select/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
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

	let expandedCommitId: string | null = $state(null);

	function firstLine(msg: string | null): string {
		if (!msg) return '-';
		return msg.split('\n')[0];
	}

	function toggleCommitExpand(id: string) {
		expandedCommitId = expandedCommitId === id ? null : id;
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
	let policies: Policy[] = $state([]);
	let repo = $state<Repo | null>(null);
	let repoName = $state('');
	let loading = $state(true);
	let policiesLoading = $state(true);
	let error = $state('');
	let policiesError = $state('');
	let syncing = $state(false);

	const repoId = $derived($page.params.id ?? '');
	const slug = $derived($page.params.slug);
	const cloneStatus = $derived(repo ? repo.clone_status : 'pending');
	let pollTimer: ReturnType<typeof setInterval> | null = $state(null);

	// Create policy dialog state
	let createOpen = $state(false);
	let createLoading = $state(false);
	let createError = $state('');
	let newName = $state('');
	let newDescription = $state('');
	let newConditionType = $state('ConditionalToolCall');
	let newToolName = $state('');
	let newMinCount = $state('1');
	let newFilePatterns = $state('');
	let newAction = $state('block_push');

	// For RequiredToolCall type
	let newToolNames = $state('');

	onMount(async () => {
		await Promise.all([loadRepo().then(() => loadCommits()), loadPolicies()]);
	});

	async function loadRepo() {
		try {
			const repos = await api.get<Repo[]>(`/api/v1/orgs/${slug}/repos`);
			repo = repos.find((r) => r.id === repoId) ?? null;
			if (repo) repoName = repo.name;
		} catch {
			// non-critical, name stays as id
		}
	}

	onDestroy(() => {
		if (pollTimer) clearInterval(pollTimer);
	});

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

	async function loadCommits() {
		try {
			const repoFilter = repoId ? `?repo_id=${repoId}` : '';
			const allCommits = await api.get<CommitListItem[]>(`/api/v1/orgs/${slug}/traces/commits${repoFilter}`);
			commits = allCommits;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load commits';
		} finally {
			loading = false;
		}
	}

	async function loadPolicies() {
		try {
			policies = await api.get<Policy[]>(`/api/v1/orgs/${slug}/repos/${repoId}/policies`);
		} catch (err) {
			policiesError = err instanceof Error ? err.message : 'Failed to load policies';
		} finally {
			policiesLoading = false;
		}
	}

	function buildCondition(): Record<string, unknown> {
		if (newConditionType === 'RequiredToolCall') {
			return {
				type: 'RequiredToolCall',
				tool_names: newToolNames
					.split(',')
					.map((s) => s.trim())
					.filter((s) => s.length > 0)
			};
		} else {
			const condition: Record<string, unknown> = {
				type: 'ConditionalToolCall',
				tool_name: newToolName,
				min_count: parseInt(newMinCount) || 1
			};
			const patterns = newFilePatterns
				.split(',')
				.map((s) => s.trim())
				.filter((s) => s.length > 0);
			if (patterns.length > 0) {
				condition.when_files_match = patterns;
			}
			return condition;
		}
	}

	async function handleCreate(e: Event) {
		e.preventDefault();
		createLoading = true;
		createError = '';
		try {
			await api.post(`/api/v1/orgs/${slug}/repos/${repoId}/policies`, {
				name: newName,
				description: newDescription || undefined,
				condition: buildCondition(),
				action: newAction
			});
			createOpen = false;
			resetCreateForm();
			await loadPolicies();
		} catch (err) {
			createError = err instanceof Error ? err.message : 'Failed to create policy';
		} finally {
			createLoading = false;
		}
	}

	function resetCreateForm() {
		newName = '';
		newDescription = '';
		newConditionType = 'ConditionalToolCall';
		newToolName = '';
		newMinCount = '1';
		newFilePatterns = '';
		newToolNames = '';
		newAction = 'block_push';
		createError = '';
	}

	async function togglePolicy(policy: Policy) {
		try {
			await api.put(`/api/v1/orgs/${slug}/policies/${policy.id}`, {
				enabled: !policy.enabled
			});
			await loadPolicies();
		} catch (err) {
			policiesError = err instanceof Error ? err.message : 'Failed to update policy';
		}
	}

	async function deletePolicy(id: string) {
		if (!confirm('Delete this policy? This cannot be undone.')) return;
		try {
			await api.delete(`/api/v1/orgs/${slug}/policies/${id}`);
			await loadPolicies();
		} catch (err) {
			policiesError = err instanceof Error ? err.message : 'Failed to delete policy';
		}
	}

	function conditionSummary(condition: Record<string, unknown>): string {
		const type = condition.type as string;
		if (type === 'RequiredToolCall') {
			const tools = (condition.tool_names as string[]) || [];
			return `Require: ${tools.join(', ')}`;
		} else if (type === 'ConditionalToolCall') {
			const tool = condition.tool_name as string;
			const min = (condition.min_count as number) || 1;
			const patterns = condition.when_files_match as string[] | undefined;
			let s = `${tool} >= ${min}x`;
			if (patterns && patterns.length > 0) {
				s += ` when ${patterns.join(', ')}`;
			}
			return s;
		}
		return JSON.stringify(condition);
	}

	function fmtTokens(n: number | null): string {
		if (n == null || n === 0) return '-';
		if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
		return String(n);
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

	<!-- Policies Section -->
	<div class="border-border overflow-hidden rounded-lg border">
		<div class="bg-muted/30 flex items-center justify-between px-4 py-3">
			<span class="text-sm font-semibold">Policies</span>
			<Dialog.Root bind:open={createOpen} onOpenChange={(open) => { if (!open) resetCreateForm(); }}>
				<Dialog.Trigger>
					{#snippet child({ props })}
						<Button size="sm" {...props}>Add Policy</Button>
					{/snippet}
				</Dialog.Trigger>
				<Dialog.Content class="sm:max-w-lg">
					<Dialog.Header>
						<Dialog.Title>Create Policy</Dialog.Title>
						<Dialog.Description>Define a tool-call requirement for this repo.</Dialog.Description>
					</Dialog.Header>
					<form onsubmit={handleCreate} class="grid gap-4">
						{#if createError}
							<p class="text-sm text-destructive">{createError}</p>
						{/if}
						<div class="grid gap-2">
							<Label for="policy_name">Name</Label>
							<Input id="policy_name" bind:value={newName} required placeholder="e.g., Code review required" />
						</div>
						<div class="grid gap-2">
							<Label for="policy_desc">Description</Label>
							<Input id="policy_desc" bind:value={newDescription} placeholder="Optional description" />
						</div>
						<div class="grid gap-2">
							<Label>Condition Type</Label>
							<Select.Root type="single" value={newConditionType} onValueChange={(v) => { if (v) newConditionType = v; }}>
								<Select.Trigger>{newConditionType === 'ConditionalToolCall' ? 'Conditional Tool Call' : 'Required Tool Call'}</Select.Trigger>
								<Select.Content>
									<Select.Item value="ConditionalToolCall">Conditional Tool Call</Select.Item>
									<Select.Item value="RequiredToolCall">Required Tool Call</Select.Item>
								</Select.Content>
							</Select.Root>
						</div>

						{#if newConditionType === 'ConditionalToolCall'}
							<div class="grid gap-2">
								<Label for="tool_name">Tool Name</Label>
								<Input id="tool_name" bind:value={newToolName} required placeholder="e.g., mcp__codex-cli__review" />
							</div>
							<div class="grid gap-2">
								<Label for="min_count">Minimum Calls</Label>
								<Input id="min_count" type="number" min="1" bind:value={newMinCount} />
							</div>
							<div class="grid gap-2">
								<Label for="file_patterns">File Patterns (comma-separated, optional)</Label>
								<Input id="file_patterns" bind:value={newFilePatterns} placeholder="e.g., src/**/*.rs, lib/**/*.ts" />
							</div>
						{:else}
							<div class="grid gap-2">
								<Label for="tool_names">Tool Names (comma-separated)</Label>
								<Input id="tool_names" bind:value={newToolNames} required placeholder="e.g., mcp__codex-cli__review, Bash" />
							</div>
						{/if}

						<div class="grid gap-2">
							<Label>Action</Label>
							<Select.Root type="single" value={newAction} onValueChange={(v) => { if (v) newAction = v; }}>
								<Select.Trigger>{newAction === 'block_push' ? 'Block Push' : 'Warn'}</Select.Trigger>
								<Select.Content>
									<Select.Item value="block_push">Block Push</Select.Item>
									<Select.Item value="warn">Warn</Select.Item>
								</Select.Content>
							</Select.Root>
						</div>

						<Dialog.Footer>
							<Button type="submit" disabled={createLoading}>
								{createLoading ? 'Creating...' : 'Create'}
							</Button>
						</Dialog.Footer>
					</form>
				</Dialog.Content>
			</Dialog.Root>
		</div>
		<div class="p-4">
			{#if policiesLoading}
				<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm">
					<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"></span>
					Loading...
				</div>
			{:else if policiesError}
				<p class="text-destructive">{policiesError}</p>
			{:else if policies.length === 0}
				<p class="text-muted-foreground text-sm">No policies configured. Add a policy to enforce tool-call requirements.</p>
			{:else}
				<Table.Root>
					<Table.Header>
						<Table.Row>
							<Table.Head class="text-xs">Name</Table.Head>
							<Table.Head class="text-xs">Condition</Table.Head>
							<Table.Head class="text-xs">Action</Table.Head>
							<Table.Head class="text-xs">Scope</Table.Head>
							<Table.Head class="text-xs">Enabled</Table.Head>
							<Table.Head class="text-xs"></Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each policies as policy}
							<Table.Row class="hover:bg-muted/40 transition-colors">
								<Table.Cell class="text-xs font-medium">{policy.name}</Table.Cell>
								<Table.Cell class="font-mono text-xs max-w-xs truncate">{conditionSummary(policy.condition)}</Table.Cell>
								<Table.Cell class="text-xs">
									{#if policy.action === 'block_push'}
										<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(240,101,101,0.12); color: #f06565; border: 1px solid rgba(240,101,101,0.25)">Block</span>
									{:else}
										<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(246,177,68,0.12); color: #f6b144; border: 1px solid rgba(246,177,68,0.25)">Warn</span>
									{/if}
								</Table.Cell>
								<Table.Cell class="text-xs">
									<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(167,139,250,0.12); color: #a78bfa; border: 1px solid rgba(167,139,250,0.25)">{policy.repo_id ? 'repo' : 'org'}</span>
								</Table.Cell>
								<Table.Cell class="text-xs">
									<Button
										variant="ghost"
										size="sm"
										onclick={() => togglePolicy(policy)}
									>
										{policy.enabled ? 'On' : 'Off'}
									</Button>
								</Table.Cell>
								<Table.Cell class="text-xs">
									{#if policy.repo_id}
										<Button variant="destructive" size="sm" onclick={() => deletePolicy(policy.id)}>
											Delete
										</Button>
									{/if}
								</Table.Cell>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
			{/if}
		</div>
	</div>

	<!-- Commits Section -->
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
