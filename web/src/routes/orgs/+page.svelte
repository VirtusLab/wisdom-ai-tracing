<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { api } from '$lib/api';

	interface OrgItem {
		org_id: string;
		org_name: string;
		display_name: string | null;
		role: string;
	}

	let orgs: OrgItem[] = $state([]);
	let loading = $state(true);
	let showCreateForm = $state(false);
	let orgName = $state('');
	let displayName = $state('');
	let creating = $state(false);
	let error = $state('');
	let showSigningKey = $state(false);
	let signingKeySeed = $state('');
	let createdOrgSlug = $state('');
	let keyCopied = $state(false);

	// GitHub org validation
	let ghOrgStatus: 'idle' | 'checking' | 'valid' | 'invalid' = $state('idle');
	let ghOrgDisplayName = $state('');
	let ghOrgAvatar = $state('');
	let ghCheckTimeout: ReturnType<typeof setTimeout> | null = null;

	function onOrgNameInput() {
		const slug = orgName.trim().toLowerCase();
		if (ghCheckTimeout) clearTimeout(ghCheckTimeout);
		ghOrgStatus = 'idle';
		ghOrgDisplayName = '';
		ghOrgAvatar = '';
		if (slug.length < 2) return;
		ghOrgStatus = 'checking';
		ghCheckTimeout = setTimeout(() => checkGithubOrg(slug), 500);
	}

	async function checkGithubOrg(slug: string) {
		try {
			const resp = await fetch(`https://api.github.com/orgs/${encodeURIComponent(slug)}`);
			if (resp.ok) {
				const data = await resp.json();
				ghOrgStatus = 'valid';
				ghOrgDisplayName = data.name || data.login;
				ghOrgAvatar = data.avatar_url || '';
				if (!displayName.trim()) displayName = ghOrgDisplayName;
			} else {
				ghOrgStatus = 'invalid';
			}
		} catch {
			ghOrgStatus = 'idle';
		}
	}

	onMount(async () => {
		const wantsCreate = $page.url.searchParams.get('create') === '1';
		try {
			orgs = await api.get<OrgItem[]>('/api/v1/me/orgs');
			if (orgs.length === 1 && !wantsCreate) {
				goto(`/orgs/${orgs[0].org_name}/dashboard`);
				return;
			}
			if (wantsCreate) showCreateForm = true;
		} catch (e) {
			console.error(e);
		}
		loading = false;
	});

	async function createOrg() {
		error = '';
		if (!orgName.trim()) {
			error = 'Organization name is required';
			return;
		}
		creating = true;
		try {
			const resp = await api.post<{ id: string; name: string; signing_key_seed?: string }>('/api/v1/orgs', {
				name: orgName.trim().toLowerCase(),
				display_name: displayName.trim() || null
			});
			if (resp.signing_key_seed) {
				signingKeySeed = resp.signing_key_seed;
				createdOrgSlug = resp.name;
				showSigningKey = true;
			} else {
				goto(`/orgs/${resp.name}/repos`);
			}
		} catch (e: any) {
			error = e.message || 'Failed to create organization';
			creating = false;
		}
	}
</script>

{#if loading}
	<div class="flex min-h-screen items-center justify-center">
		<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm"><span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"></span>Loading...</div>
	</div>
{:else if showSigningKey}
	<div class="flex min-h-screen items-center justify-center">
		<div class="w-full max-w-lg space-y-6 p-8">
			<h1 class="text-2xl font-bold">Save Your Signing Key</h1>
			<p class="text-muted-foreground">Your organization <strong>{createdOrgSlug}</strong> has been created. Save this signing key now — it will not be shown again.</p>

			<div class="rounded-md border border-destructive bg-destructive/5 p-4 space-y-1">
				<p class="text-sm font-semibold text-destructive">One-time display</p>
				<p class="text-sm text-destructive/90">This key is stored encrypted on the server and cannot be retrieved later. Save it in a secure location.</p>
			</div>

			<div class="space-y-2">
				<p class="text-sm font-medium">Signing Key (Ed25519 seed, base64)</p>
				<code class="block w-full rounded-md border bg-muted p-3 text-xs font-mono break-all select-all">
					{signingKeySeed}
				</code>
				<div class="flex gap-2">
					<button
						onclick={() => {
							navigator.clipboard.writeText(signingKeySeed);
							keyCopied = true;
							setTimeout(() => (keyCopied = false), 2000);
						}}
						class="inline-flex h-8 items-center rounded-md border px-3 text-xs font-medium hover:bg-muted"
					>
						{keyCopied ? 'Copied!' : 'Copy to clipboard'}
					</button>
					<button
						onclick={() => {
							const blob = new Blob([signingKeySeed], { type: 'text/plain' });
							const url = URL.createObjectURL(blob);
							const a = document.createElement('a');
							a.href = url;
							a.download = `${createdOrgSlug}-signing-key.txt`;
							a.click();
							URL.revokeObjectURL(url);
						}}
						class="inline-flex h-8 items-center rounded-md border px-3 text-xs font-medium hover:bg-muted"
					>
						Download as file
					</button>
				</div>
			</div>

			<button
				onclick={() => goto(`/orgs/${createdOrgSlug}/repos`)}
				class="inline-flex h-10 w-full items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90"
			>
				I've saved my key — Continue to dashboard
			</button>
		</div>
	</div>
{:else}
	<div class="flex min-h-screen items-center justify-center">
		<div class="w-full max-w-lg space-y-6 p-8">
			{#if orgs.length > 0}
				<div>
					<h1 class="text-2xl font-bold">Your Organizations</h1>
				</div>
				<div class="grid gap-3">
					{#each orgs as org}
						<a href="/orgs/{org.org_name}/repos" class="border-border flex items-center justify-between overflow-hidden rounded-lg border p-4 hover:bg-muted/40 transition-colors">
							<div>
								<div class="font-semibold">{org.display_name || org.org_name}</div>
								{#if org.display_name}
									<div class="text-xs text-muted-foreground">{org.org_name}</div>
								{/if}
							</div>
							<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(167,139,250,0.12); color: #a78bfa; border: 1px solid rgba(167,139,250,0.25)">{org.role}</span>
						</a>
					{/each}
				</div>

				{#if !showCreateForm}
					<button
						onclick={() => (showCreateForm = true)}
						class="text-sm text-muted-foreground hover:text-foreground underline"
					>
						Create another organization
					</button>
				{/if}
			{:else}
				<div>
					<h1 class="text-2xl font-bold">No Organizations</h1>
					<p class="text-muted-foreground mt-1">You are not a member of any organization yet. Create one or ask an admin to invite you.</p>
				</div>
			{/if}

			{#if showCreateForm || orgs.length === 0}
				<form onsubmit={(e) => { e.preventDefault(); createOrg(); }} class="space-y-4 border-t pt-4">
					<h2 class="text-sm font-semibold uppercase tracking-wide text-muted-foreground">Create Organization</h2>
					<div class="space-y-2">
						<label for="orgName" class="text-sm font-medium">GitHub organization</label>
						<input
							id="orgName"
							type="text"
							bind:value={orgName}
							oninput={onOrgNameInput}
							placeholder="my-github-org"
							class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
							required
						/>
						{#if ghOrgStatus === 'checking'}
							<p class="text-xs text-muted-foreground">Checking GitHub...</p>
						{:else if ghOrgStatus === 'valid'}
							<div class="flex items-center gap-2 text-xs text-green-600">
								{#if ghOrgAvatar}
									<img src={ghOrgAvatar} alt="" class="h-5 w-5 rounded" />
								{/if}
								<span>{ghOrgDisplayName}</span>
							</div>
						{:else if ghOrgStatus === 'invalid'}
							<p class="text-xs text-destructive">Organization not found on GitHub.</p>
						{:else}
							<p class="text-xs text-muted-foreground">
								Must match your GitHub organization name exactly (lowercase).
							</p>
						{/if}
					</div>

					<div class="space-y-2">
						<label for="displayName" class="text-sm font-medium">Display name (optional)</label>
						<input
							id="displayName"
							type="text"
							bind:value={displayName}
							placeholder="My Organization"
							class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
						/>
					</div>

					{#if error}
						<p class="text-sm text-destructive">{error}</p>
					{/if}

					<button
						type="submit"
						disabled={creating || ghOrgStatus === 'invalid' || ghOrgStatus === 'checking'}
						class="inline-flex h-10 w-full items-center justify-center rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
					>
						{creating ? 'Creating...' : 'Create Organization'}
					</button>
				</form>
			{/if}
		</div>
	</div>
{/if}
