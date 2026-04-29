<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { api } from '$lib/api';
	import { orgStore } from '$lib/stores/org';
	import { features } from '$lib/stores/features';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Alert from '$lib/components/ui/alert/index.js';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import { formatDate } from '$lib/utils/date';

	interface OrgDetail {
		id: string;
		name: string;
		display_name: string | null;
		created_at: string;
	}

	const slug = $derived($page.params.slug);

	let orgState: { current: { role: string } | null } = $state({ current: null });
	orgStore.subscribe((s) => (orgState = s));

	let org: OrgDetail | null = $state(null);
	let editing = $state(false);
	let editDisplayName = $state('');
	let editingName = $state(false);
	let editName = $state('');
	let saving = $state(false);
	let savingName = $state(false);
	let error = $state('');
	let nameError = $state('');
	let success = $state('');

	const isOwner = $derived(orgState.current?.role === 'owner');

	onMount(async () => {
		try {
			org = await api.get<OrgDetail>(`/api/v1/orgs/${slug}`);
			editDisplayName = org.display_name ?? '';
			editName = org.name;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load organization';
		}
	});

	async function handleSave() {
		if (!org) return;
		saving = true;
		error = '';
		success = '';
		try {
			await api.put(`/api/v1/orgs/${slug}`, { display_name: editDisplayName || null });
			org.display_name = editDisplayName || null;
			editing = false;
			success = 'Organization updated.';
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to update';
		} finally {
			saving = false;
		}
	}

	async function handleSaveName() {
		if (!org) return;
		const next = editName.trim();
		if (!next || next === org.name) {
			editingName = false;
			return;
		}
		savingName = true;
		nameError = '';
		success = '';
		try {
			const resp = await api.put<{ name: string }>(`/api/v1/orgs/${slug}`, { name: next });
			editingName = false;
			await goto(`/orgs/${resp.name}/settings/org`, { invalidateAll: true });
		} catch (err) {
			nameError = err instanceof Error ? err.message : 'Failed to rename';
		} finally {
			savingName = false;
		}
	}
</script>

<svelte:head>
	<title>{slug} - Settings - TraceVault</title>
</svelte:head>

<div class="space-y-6">
	<div class="flex items-center gap-2">
		<a href="/orgs/{slug}/settings" class="text-muted-foreground hover:text-foreground">Organizations</a>
		<span class="text-muted-foreground">/</span>
		<h1 class="text-2xl font-bold">{org?.display_name || slug}</h1>
	</div>

	<div class="flex gap-2 text-sm border-b pb-2">
		<a href="/orgs/{slug}/settings/org" class="font-semibold underline">General</a>
		<a href="/orgs/{slug}/settings/members" class="text-muted-foreground hover:underline">Members</a>
		<a href="/orgs/{slug}/settings/api-keys" class="text-muted-foreground hover:underline">API Keys</a>
		{#if $features.sso}
			<a href="/orgs/{slug}/settings/sso" class="text-muted-foreground hover:underline">SSO</a>
		{/if}
	</div>

	{#if error}
		<ErrorState message={error} />
	{:else}

	{#if success}
		<Alert.Root>
			<Alert.Title>Success</Alert.Title>
			<Alert.Description>{success}</Alert.Description>
		</Alert.Root>
	{/if}

	{#if org}
		<div class="border-border overflow-hidden rounded-lg border max-w-lg">
			<div class="bg-muted/30 px-4 py-3 text-sm font-semibold">General</div>
			<div class="p-4 space-y-4">
				{#if editingName}
					<div class="space-y-3">
						<div class="grid gap-2">
							<Label for="org_name">GitHub Organization</Label>
							<Input id="org_name" bind:value={editName} placeholder="e.g. VirtusLab" />
							<p class="text-xs text-muted-foreground">
								Must match your GitHub organization name (case is preserved). Allowed: letters, digits, hyphens.
								Renaming changes the URL of this org and may break repo configs that reference the old slug.
							</p>
							{#if nameError}
								<p class="text-xs text-destructive">{nameError}</p>
							{/if}
						</div>
						<div class="flex gap-2">
							<Button onclick={handleSaveName} disabled={savingName}>
								{savingName ? 'Saving...' : 'Save'}
							</Button>
							<Button variant="outline" onclick={() => { editingName = false; nameError = ''; editName = org?.name ?? ''; }}>
								Cancel
							</Button>
						</div>
					</div>
				{:else}
					<div class="flex justify-between items-center gap-2">
						<div>
							<span class="text-muted-foreground text-xs">GitHub Organization</span>
							<p class="font-mono text-sm">{org.name}</p>
						</div>
						<div class="flex items-center gap-3">
							<a
								href="https://github.com/{org.name}"
								target="_blank"
								rel="noopener noreferrer"
								class="text-xs text-muted-foreground hover:underline"
							>
								View on GitHub
							</a>
							{#if isOwner}
								<Button variant="outline" size="sm" onclick={() => (editingName = true)}>Edit</Button>
							{/if}
						</div>
					</div>
				{/if}

				<div class="border-t pt-4">
					{#if editing}
						<div class="space-y-3">
							<div class="grid gap-2">
								<Label for="display_name">Display Name</Label>
								<Input id="display_name" bind:value={editDisplayName} placeholder="e.g. My Company" />
								<p class="text-xs text-muted-foreground">A human-friendly name shown in the UI. Leave empty to use the GitHub org name.</p>
							</div>
							<div class="flex gap-2">
								<Button onclick={handleSave} disabled={saving}>
									{saving ? 'Saving...' : 'Save'}
								</Button>
								<Button variant="outline" onclick={() => { editing = false; editDisplayName = org?.display_name ?? ''; }}>
									Cancel
								</Button>
							</div>
						</div>
					{:else}
						<div class="flex justify-between items-center">
							<div>
								<span class="text-muted-foreground text-xs">Display Name</span>
								<p class="text-sm">{org.display_name || '—'}</p>
							</div>
							{#if isOwner}
								<Button variant="outline" size="sm" onclick={() => (editing = true)}>Edit</Button>
							{/if}
						</div>
					{/if}
				</div>

				<div class="border-t pt-4">
					<div class="flex items-center justify-between py-1.5 text-sm">
						<span class="text-muted-foreground text-xs">Created</span>
						<span class="text-xs">{formatDate(org.created_at)}</span>
					</div>
				</div>
			</div>
		</div>
	{/if}
	{/if}
</div>
