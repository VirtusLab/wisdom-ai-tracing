<script lang="ts">
	import { page } from '$app/stores';
	import { api } from '$lib/api';
	import { orgStore } from '$lib/stores/org';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Alert from '$lib/components/ui/alert/index.js';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import * as Select from '$lib/components/ui/select/index.js';

	interface LlmSettings {
		provider: string | null;
		has_api_key: boolean;
		model: string | null;
		base_url: string | null;
	}

	const slug = $derived($page.params.slug);

	let orgState: { current: { role: string } | null } = $state({ current: null });
	orgStore.subscribe((s) => (orgState = s));

	let settings: LlmSettings | null = $state(null);
	let loading = $state(true);
	let editing = $state(false);
	let saving = $state(false);
	let error = $state('');
	let success = $state('');

	let editProvider = $state('anthropic');
	let editApiKey = $state('');
	let editModel = $state('');
	let editBaseUrl = $state('');

	$effect(() => {
		if (slug) loadSettings();
	});

	async function loadSettings() {
		loading = true;
		error = '';
		try {
			settings = await api.get<LlmSettings>(
				`/api/v1/orgs/${slug}/llm-settings`
			);
			editProvider = settings.provider ?? 'anthropic';
			editModel = settings.model ?? '';
			editBaseUrl = settings.base_url ?? '';
			editApiKey = '';
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load Stories LLM settings';
		} finally {
			loading = false;
		}
	}

	function startEditing() {
		editing = true;
		success = '';
		error = '';
	}

	function cancelEditing() {
		editing = false;
		editProvider = settings?.provider ?? 'anthropic';
		editModel = settings?.model ?? '';
		editBaseUrl = settings?.base_url ?? '';
		editApiKey = '';
	}

	async function handleSave() {
		saving = true;
		error = '';
		success = '';
		try {
			const body: Record<string, string> = {
				provider: editProvider
			};
			if (editApiKey) body.api_key = editApiKey;
			if (editModel) body.model = editModel;
			if (editBaseUrl) body.base_url = editBaseUrl;

			await api.put(`/api/v1/orgs/${slug}/llm-settings`, body);
			editing = false;
			success = 'Stories LLM settings saved.';
			await loadSettings();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to save Stories LLM settings';
		} finally {
			saving = false;
		}
	}

	const isOwnerOrAdmin = $derived(
		orgState.current?.role === 'owner' || orgState.current?.role === 'admin'
	);
</script>

<svelte:head>
	<title>Stories LLM Settings - TraceVault</title>
</svelte:head>

<div class="space-y-6">
	<h1 class="text-2xl font-bold">Stories LLM Configuration</h1>
	<p class="text-muted-foreground text-sm">Configure the AI model provider used for story generation across the platform.</p>

	{#if error}
		<ErrorState message={error} />
	{:else}

	{#if success}
		<Alert.Root>
			<Alert.Title>Success</Alert.Title>
			<Alert.Description>{success}</Alert.Description>
		</Alert.Root>
	{/if}

	{#if loading}
		<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm">
			<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"></span>
			Loading...
		</div>
	{:else}
		<div class="border-border overflow-hidden rounded-lg border max-w-lg">
			<div class="bg-muted/30 px-4 py-3">
				<span class="text-sm font-semibold">LLM Provider</span>
				<p class="text-xs text-muted-foreground mt-0.5">
					Configure the AI model used for code story generation.
				</p>
			</div>
			<div class="p-4 space-y-4">
				{#if editing}
					<div class="grid gap-2">
						<Label>Provider</Label>
						<Select.Root type="single" value={editProvider} onValueChange={(v) => editProvider = v}>
							<Select.Trigger class="w-full">
								<span data-slot="select-value">{editProvider === 'anthropic' ? 'Anthropic' : 'OpenAI'}</span>
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="anthropic">Anthropic</Select.Item>
								<Select.Item value="openai">OpenAI</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>
					<div class="grid gap-2">
						<Label for="llm_api_key">API Key</Label>
						<Input
							id="llm_api_key"
							type="password"
							bind:value={editApiKey}
							placeholder={settings?.has_api_key
								? 'Leave blank to keep current key'
								: 'Enter API key'}
						/>
						{#if settings?.has_api_key && !editApiKey}
							<p class="text-xs text-muted-foreground">
								A key is already configured. Leave blank to keep it.
							</p>
						{/if}
					</div>
					<div class="grid gap-2">
						<Label for="llm_model">Model</Label>
						<Input
							id="llm_model"
							bind:value={editModel}
							placeholder={editProvider === 'anthropic'
								? 'claude-sonnet-4-20250514'
								: 'gpt-4o'}
						/>
						<p class="text-xs text-muted-foreground">Leave blank for default.</p>
					</div>
					<div class="grid gap-2">
						<Label for="llm_base_url">Base URL (optional)</Label>
						<Input
							id="llm_base_url"
							bind:value={editBaseUrl}
							placeholder="https://api.anthropic.com"
						/>
					</div>
					<div class="flex gap-2">
						<Button onclick={handleSave} disabled={saving}>
							{saving ? 'Saving...' : 'Save'}
						</Button>
						<Button variant="outline" onclick={cancelEditing}>Cancel</Button>
					</div>
				{:else}
					<div class="flex items-center justify-between py-1.5 text-sm">
						<span class="text-muted-foreground text-xs">Provider</span>
						<span class="text-xs capitalize">{settings?.provider ?? 'Not configured'}</span>
					</div>
					<div class="flex items-center justify-between py-1.5 text-sm">
						<span class="text-muted-foreground text-xs">API Key</span>
						{#if settings?.has_api_key}
							<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(62,207,142,0.12); color: #3ecf8e; border: 1px solid rgba(62,207,142,0.25)">Configured</span>
						{:else}
							<span class="text-xs text-muted-foreground">Not set</span>
						{/if}
					</div>
					<div class="flex items-center justify-between py-1.5 text-sm">
						<span class="text-muted-foreground text-xs">Model</span>
						<span class="text-xs">{settings?.model ?? 'Default'}</span>
					</div>
					{#if settings?.base_url}
						<div class="flex items-center justify-between py-1.5 text-sm">
							<span class="text-muted-foreground text-xs">Base URL</span>
							<span class="text-xs truncate max-w-[200px]">{settings.base_url}</span>
						</div>
					{/if}
					{#if isOwnerOrAdmin}
						<Button variant="outline" onclick={startEditing}>Edit</Button>
					{/if}
				{/if}
			</div>
		</div>
	{/if}
	{/if}
</div>
