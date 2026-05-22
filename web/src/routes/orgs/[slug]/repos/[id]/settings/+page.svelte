<script lang="ts">
	import { page } from '$app/stores';
	import { onMount, onDestroy } from 'svelte';
	import { api } from '$lib/api';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Select from '$lib/components/ui/select/index.js';
	import { formatDateTime } from '$lib/utils/date';

	interface RepoSettings {
		github_url: string | null;
		clone_status: string;
		has_deploy_key: boolean;
		last_fetched_at: string | null;
		validation_window_mode: string;
	}

	interface Repo {
		id: string;
		name: string;
		github_url: string | null;
		clone_status: string;
		created_at: string;
	}

	const repoId = $derived($page.params.id);
	const slug = $derived($page.params.slug);

	let settings: RepoSettings | null = $state(null);
	let repoName = $state('');
	let loading = $state(true);
	let saving = $state(false);
	let error = $state('');
	let success = $state('');

	let githubUrl = $state('');
	let deployKey = $state('');
	let validationWindowMode = $state('disabled');
	let pollTimer: ReturnType<typeof setInterval> | null = $state(null);

	onMount(async () => {
		await Promise.all([loadSettings(), loadRepoName()]);
	});

	onDestroy(() => {
		if (pollTimer) clearInterval(pollTimer);
	});

	async function loadRepoName() {
		try {
			const repos = await api.get<Repo[]>(`/api/v1/orgs/${slug}/repos`);
			const repo = repos.find((r) => r.id === repoId);
			if (repo) repoName = repo.name;
		} catch {
			// non-critical
		}
	}

	async function loadSettings() {
		try {
			settings = await api.get<RepoSettings>(`/api/v1/orgs/${slug}/repos/${repoId}/settings`);
			githubUrl = settings.github_url ?? '';
			validationWindowMode = settings.validation_window_mode ?? 'disabled';
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load settings';
		} finally {
			loading = false;
		}
	}

	async function handleSave() {
		saving = true;
		error = '';
		success = '';
		try {
			const body: Record<string, string> = {};
			if (githubUrl) body.github_url = githubUrl;
			if (deployKey.trim()) body.deploy_key = deployKey.trim();
			body.validation_window_mode = validationWindowMode;

			settings = await api.put<RepoSettings>(`/api/v1/orgs/${slug}/repos/${repoId}/settings`, body);

			deployKey = '';
			success = 'Settings saved.';

			if (settings.clone_status === 'cloning') {
				startPolling();
			}
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to save settings';
		} finally {
			saving = false;
		}
	}

	function startPolling() {
		if (pollTimer) return;
		pollTimer = setInterval(async () => {
			try {
				settings = await api.get<RepoSettings>(`/api/v1/orgs/${slug}/repos/${repoId}/settings`);
				if (settings.clone_status !== 'cloning') {
					if (pollTimer) clearInterval(pollTimer);
					pollTimer = null;
				}
			} catch {
				// ignore polling errors
			}
		}, 3000);
	}

	async function handleSync() {
		saving = true;
		error = '';
		success = '';
		try {
			const result = await api.post<{ status: string }>(`/api/v1/orgs/${slug}/repos/${repoId}/sync`);
			if (result.status === 'cloning') {
				if (settings) settings.clone_status = 'cloning';
				startPolling();
			} else {
				await loadSettings();
				success = 'Repository synced.';
			}
		} catch (err) {
			error = err instanceof Error ? err.message : 'Sync failed';
		} finally {
			saving = false;
		}
	}

</script>

<svelte:head>
	<title>Repo Settings - TraceVault</title>
</svelte:head>

<div class="space-y-6">
	<div class="flex items-center gap-2">
		<a href="/orgs/{slug}/repos" class="text-muted-foreground hover:underline">Repos</a>
		<span class="text-muted-foreground">/</span>
		<a href="/orgs/{slug}/repos/{repoId}" class="text-muted-foreground hover:underline">{repoName || repoId}</a>
		<span class="text-muted-foreground">/</span>
		<h1 class="text-2xl font-bold">Settings</h1>
	</div>

	{#if loading}
		<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm">
			<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"></span>
			Loading...
		</div>
	{:else if settings}
		{#if error}
			<div class="rounded-md border border-destructive bg-destructive/10 p-3 text-sm text-destructive">
				{error}
			</div>
		{/if}
		{#if success}
			<div class="rounded-md border border-green-500/50 bg-green-500/10 p-3 text-sm text-green-700 dark:text-green-400">
				{success}
			</div>
		{/if}

		<!-- GitHub Connection -->
		<div class="border-border overflow-hidden rounded-lg border">
			<div class="bg-muted/30 px-4 py-3">
				<span class="text-sm font-semibold">GitHub Connection</span>
				<p class="text-xs text-muted-foreground mt-0.5">SSH URL for cloning the repository.</p>
			</div>
			<div class="p-4 space-y-3">
				<div class="grid gap-2">
					<Label for="github_url">Repository URL (SSH)</Label>
					<Input
						id="github_url"
						bind:value={githubUrl}
						placeholder="git@github.com:org/repo.git"
					/>
					<p class="text-xs text-muted-foreground">
						Use SSH format: <code class="rounded bg-muted px-1">git@github.com:org/repo.git</code>
					</p>
				</div>
			</div>
		</div>

		<!-- Deploy Key -->
		<div class="border-border overflow-hidden rounded-lg border">
			<div class="bg-muted/30 px-4 py-3">
				<span class="text-sm font-semibold flex items-center gap-2">
					Deploy Key
					{#if settings.has_deploy_key}
						<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(62,207,142,0.12); color: #3ecf8e; border: 1px solid rgba(62,207,142,0.25)">Configured</span>
					{/if}
				</span>
				<p class="text-xs text-muted-foreground mt-0.5">SSH deploy key for accessing private repositories.</p>
			</div>
			<div class="p-4 space-y-4">
				<div class="rounded-md border bg-muted/50 p-4 text-sm space-y-3">
					<p class="font-medium">Setup instructions:</p>
					<ol class="list-decimal list-inside space-y-2 text-muted-foreground">
						<li>
							Generate an ed25519 key pair locally:
							<code class="block mt-1 rounded bg-muted px-2 py-1 text-xs font-mono">ssh-keygen -t ed25519 -f tracevault-deploy-key -N ""</code>
						</li>
						<li>
							In GitHub, go to your repo's <strong>Settings → Deploy keys → Add deploy key</strong>,
							paste the contents of <code class="rounded bg-muted px-1 text-xs">tracevault-deploy-key.pub</code>
							(read-only access is sufficient).
						</li>
						<li>
							Paste the <strong>private key</strong> contents (<code class="rounded bg-muted px-1 text-xs">tracevault-deploy-key</code> file) below.
						</li>
					</ol>
				</div>
				<div class="grid gap-2">
					<Label for="deploy_key">
						{settings.has_deploy_key ? 'Replace deploy key' : 'Private key (PEM)'}
					</Label>
					<textarea
						id="deploy_key"
						bind:value={deployKey}
						placeholder="-----BEGIN OPENSSH PRIVATE KEY-----&#10;...&#10;-----END OPENSSH PRIVATE KEY-----"
						rows="6"
						class="flex w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring font-mono"
					></textarea>
				</div>
			</div>
		</div>

		<!-- Sync Status -->
		<div class="border-border overflow-hidden rounded-lg border">
			<div class="bg-muted/30 px-4 py-3 text-sm font-semibold">Sync Status</div>
			<div class="p-4 space-y-3">
				<div class="flex items-center justify-between py-1.5 text-sm">
					<span class="text-muted-foreground text-xs">Clone status</span>
					<span class="text-xs">
						{#if settings.clone_status === 'ready'}
							<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(62,207,142,0.12); color: #3ecf8e; border: 1px solid rgba(62,207,142,0.25)">Ready</span>
						{:else if settings.clone_status === 'cloning'}
							<span class="inline-flex items-center rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(167,139,250,0.12); color: #a78bfa; border: 1px solid rgba(167,139,250,0.25)">
								<svg class="mr-1 h-3 w-3 animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<path d="M12 2v4m0 12v4m-7.07-3.93l2.83-2.83m8.48-8.48l2.83-2.83M2 12h4m12 0h4m-3.93 7.07l-2.83-2.83M6.34 6.34L3.51 3.51" />
								</svg>
								Cloning...
							</span>
						{:else if settings.clone_status === 'error'}
							<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(240,101,101,0.12); color: #f06565; border: 1px solid rgba(240,101,101,0.25)">Error</span>
						{:else}
							<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(79,110,247,0.12); color: #4f6ef7; border: 1px solid rgba(79,110,247,0.25)">Not cloned</span>
						{/if}
					</span>
				</div>
				<div class="flex items-center justify-between py-1.5 text-sm">
					<span class="text-muted-foreground text-xs">Last fetched</span>
					<span class="text-xs">{formatDateTime(settings.last_fetched_at)}</span>
				</div>
				<div class="flex items-center gap-3">
					{#if settings.clone_status === 'ready'}
						<Button variant="outline" size="sm" onclick={handleSync} disabled={saving}>
							Sync Now
						</Button>
						<a
							href="/orgs/{slug}/repos/{repoId}/code"
							class="inline-flex items-center gap-2 rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
						>
							Browse Code
						</a>
					{/if}
				</div>
			</div>
		</div>

		<!-- Validation Window Mode -->
		<div class="border-border overflow-hidden rounded-lg border">
			<div class="bg-muted/30 px-4 py-3 text-sm font-semibold">Validation Window</div>
			<div class="p-4 space-y-3">
				<p class="text-xs text-muted-foreground">
					Controls what happens when an unknown tool (not covered by any
					<code>validation_window</code>-scoped policy) is called inside a declared validation window.
					Agents open a window with <code>tracevault validation-start</code>.
				</p>
				<div class="grid gap-2">
					<Label>Unknown Tool Mode</Label>
					<Select.Root type="single" value={validationWindowMode} onValueChange={(v) => { if (v) validationWindowMode = v; }}>
						<Select.Trigger>
							{{ disabled: 'Disabled', warn: 'Warn', block: 'Block Push' }[validationWindowMode] ?? validationWindowMode}
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="disabled">Disabled</Select.Item>
							<Select.Item value="warn">Warn</Select.Item>
							<Select.Item value="block">Block Push</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>
			</div>
		</div>

		<!-- Save -->
		<div class="flex justify-end">
			<Button onclick={handleSave} disabled={saving}>
				{saving ? 'Saving...' : 'Save Settings'}
			</Button>
		</div>
	{/if}
</div>
