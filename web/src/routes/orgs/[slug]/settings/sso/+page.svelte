<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { api } from '$lib/api';
	import { orgStore } from '$lib/stores/org';
	import { features } from '$lib/stores/features';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Alert from '$lib/components/ui/alert/index.js';
	import EnterpriseUpgrade from '$lib/components/enterprise-upgrade.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';

	interface SsoConfig {
		issuer_url: string;
		client_id: string;
		allowed_domains: string[];
		enforce: boolean;
		auto_provision: boolean;
		default_role: string;
		linked_users: number;
		client_secret_set: boolean;
	}

	const slug = $derived($page.params.slug);

	let orgState: { current: { role: string } | null } = $state({ current: null });
	orgStore.subscribe((s) => (orgState = s));

	let featureFlags: { sso: boolean } = $state({ sso: false });
	features.subscribe((f) => (featureFlags = f));

	let config: SsoConfig | null = $state(null);
	let loading = $state(true);
	let error = $state('');
	let success = $state('');
	let saving = $state(false);
	let deleting = $state(false);
	let notConfigured = $state(false);

	// Form fields
	let issuerUrl = $state('');
	let clientId = $state('');
	let clientSecret = $state('');
	let domainsInput = $state('');
	let enforce = $state(true);
	let autoProvision = $state(true);
	let defaultRole = $state('developer');

	const canManageSso = $derived(
		orgState.current?.role === 'owner' || orgState.current?.role === 'admin'
	);

	onMount(async () => {
		try {
			config = await api.get<SsoConfig>(`/api/v1/orgs/${slug}/sso`);
			issuerUrl = config.issuer_url;
			clientId = config.client_id;
			domainsInput = config.allowed_domains.join(', ');
			enforce = config.enforce;
			autoProvision = config.auto_provision;
			defaultRole = config.default_role;
		} catch (err: any) {
			if (err.message?.includes('not configured') || err.message?.includes('Not found')) {
				notConfigured = true;
			} else {
				error = err.message || 'Failed to load SSO config';
			}
		}
		loading = false;
	});

	async function handleSave() {
		error = '';
		success = '';
		saving = true;

		const domains = domainsInput
			.split(',')
			.map((d) => d.trim())
			.filter((d) => d.length > 0);

		if (domains.length === 0) {
			error = 'At least one allowed domain is required';
			saving = false;
			return;
		}

		const body: Record<string, unknown> = {
			issuer_url: issuerUrl,
			client_id: clientId,
			allowed_domains: domains,
			enforce,
			auto_provision: autoProvision,
			default_role: defaultRole
		};

		if (clientSecret) {
			body.client_secret = clientSecret;
		}

		try {
			await api.put(`/api/v1/orgs/${slug}/sso`, body);
			success = 'SSO configuration saved.';
			notConfigured = false;
			clientSecret = '';

			config = await api.get<SsoConfig>(`/api/v1/orgs/${slug}/sso`);
		} catch (err: any) {
			error = err.message || 'Failed to save SSO config';
		} finally {
			saving = false;
		}
	}

	async function handleDelete() {
		if (!confirm('Are you sure you want to remove SSO? Users without passwords will lose access.')) {
			return;
		}
		error = '';
		success = '';
		deleting = true;
		try {
			const resp = await api.delete<{ affected_passwordless_users: number }>(
				`/api/v1/orgs/${slug}/sso`
			);
			config = null;
			notConfigured = true;
			issuerUrl = '';
			clientId = '';
			clientSecret = '';
			domainsInput = '';
			enforce = true;
			autoProvision = true;
			defaultRole = 'developer';
			success = `SSO removed. ${resp.affected_passwordless_users} user(s) without passwords have lost access.`;
		} catch (err: any) {
			error = err.message || 'Failed to remove SSO config';
		} finally {
			deleting = false;
		}
	}
</script>

<svelte:head>
	<title>{slug} - SSO Settings - TraceVault</title>
</svelte:head>

<div class="space-y-6">
	<div class="flex items-center gap-2">
		<a href="/orgs/{slug}/settings" class="text-muted-foreground hover:text-foreground">Organizations</a>
		<span class="text-muted-foreground">/</span>
		<h1 class="text-2xl font-bold">SSO Configuration</h1>
	</div>

	<div class="flex gap-2 text-sm border-b pb-2">
		<a href="/orgs/{slug}/settings/org" class="text-muted-foreground hover:underline">General</a>
		<a href="/orgs/{slug}/settings/members" class="text-muted-foreground hover:underline">Members</a>
		<a href="/orgs/{slug}/settings/api-keys" class="text-muted-foreground hover:underline">API Keys</a>
		<a href="/orgs/{slug}/settings/sso" class="font-semibold underline">SSO</a>
	</div>

	{#if !featureFlags.sso}
		<EnterpriseUpgrade feature="sso" />
	{:else if loading}
		<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm">
			<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"></span>
			Loading...
		</div>
	{:else}
		{#if error}
			<Alert.Root variant="destructive">
				<Alert.Title>Error</Alert.Title>
				<Alert.Description>{error}</Alert.Description>
			</Alert.Root>
		{/if}

		{#if success}
			<Alert.Root>
				<Alert.Title>Success</Alert.Title>
				<Alert.Description>{success}</Alert.Description>
			</Alert.Root>
		{/if}

		<div class="border-border overflow-hidden rounded-lg border max-w-lg">
			<div class="bg-muted/30 px-4 py-3 text-sm font-semibold flex items-center justify-between">
				<span>OIDC Single Sign-On</span>
				{#if config}
					<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(62,207,142,0.12); color: #3ecf8e; border: 1px solid rgba(62,207,142,0.25)">
						Active — {config.linked_users} linked user{config.linked_users === 1 ? '' : 's'}
					</span>
				{/if}
			</div>
			<div class="p-4 space-y-4">
				{#if !canManageSso}
					<p class="text-sm text-muted-foreground">Only organization owners and admins can configure SSO.</p>
				{:else}
					<form onsubmit={(e) => { e.preventDefault(); handleSave(); }} class="space-y-4">
						<div class="grid gap-2">
							<Label for="issuer">Issuer URL</Label>
							<Input id="issuer" bind:value={issuerUrl} placeholder="https://accounts.google.com" required />
							<p class="text-xs text-muted-foreground">The OIDC issuer URL of your identity provider.</p>
						</div>
						<div class="grid gap-2">
							<Label for="clientId">Client ID</Label>
							<Input id="clientId" bind:value={clientId} required />
						</div>
						<div class="grid gap-2">
							<Label for="clientSecret">Client Secret</Label>
							<Input
								id="clientSecret"
								type="password"
								bind:value={clientSecret}
								placeholder={config ? '••••••••' : ''}
								required={!config}
							/>
							{#if config}
								<p class="text-xs text-muted-foreground">Leave empty to keep the existing secret.</p>
							{/if}
						</div>
						<div class="grid gap-2">
							<Label for="domains">Allowed Email Domains</Label>
							<Input id="domains" bind:value={domainsInput} placeholder="company.com, company.co.uk" required />
							<p class="text-xs text-muted-foreground">Comma-separated list of email domains allowed for SSO login.</p>
						</div>
						<div class="flex items-center gap-3">
							<input type="checkbox" id="enforce" bind:checked={enforce} class="h-4 w-4 rounded border" />
							<Label for="enforce">Enforce SSO (disable password login for non-admin members)</Label>
						</div>
						<div class="flex items-center gap-3">
							<input type="checkbox" id="autoProvision" bind:checked={autoProvision} class="h-4 w-4 rounded border" />
							<Label for="autoProvision">Auto-provision users on first SSO login</Label>
						</div>
						<div class="grid gap-2">
							<Label for="defaultRole">Default Role for New Users</Label>
							<select id="defaultRole" bind:value={defaultRole} class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm">
								<option value="developer">Developer</option>
								<option value="admin">Admin</option>
								<option value="auditor">Auditor</option>
							</select>
						</div>
						<div class="flex gap-2">
							<Button type="submit" disabled={saving}>
								{saving ? 'Saving...' : config ? 'Update SSO' : 'Enable SSO'}
							</Button>
							{#if config}
								<Button variant="destructive" onclick={handleDelete} disabled={deleting}>
									{deleting ? 'Removing...' : 'Remove SSO'}
								</Button>
							{/if}
						</div>
					</form>
				{/if}
			</div>
		</div>
	{/if}
</div>
