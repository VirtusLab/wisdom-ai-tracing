<script lang="ts">
	import { api } from '$lib/api';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Alert from '$lib/components/ui/alert/index.js';
	import * as Table from '$lib/components/ui/table/index.js';
	import * as Select from '$lib/components/ui/select/index.js';
	import ErrorState from '$lib/components/ErrorState.svelte';

	interface Credential {
		name: string;
		protocol: string;
		base_url: string;
		max_concurrent: number;
		configured_at: string;
	}

	interface Rule {
		id: string;
		match_model: string | null;
		credential_name: string;
		provider_model: string | null;
	}

	const DEFAULT_BASE_URL = 'https://api.anthropic.com';
	const DEFAULT_MAX_CONCURRENT = 8;
	const MIN_MAX_CONCURRENT = 1;
	const MAX_MAX_CONCURRENT = 256;
	const KEY_PREFIX = 'sk-ant-';
	const KEY_MAX_LEN = 256;

	let credentials = $state<Credential[]>([]);
	let rules = $state<Rule[]>([]);
	let loading = $state(true);
	let error = $state('');
	let success = $state('');

	// --- Add/update credential form ---
	let credName = $state('');
	let credBaseUrl = $state(DEFAULT_BASE_URL);
	let credKey = $state('');
	let credMaxConcurrent: number = $state(DEFAULT_MAX_CONCURRENT);
	let savingCred = $state(false);
	let credError = $state('');

	// Per-credential delete confirmation (keyed by name).
	let confirmingDelete = $state<string | null>(null);
	let deletingCred = $state(false);

	// Per-credential edit form (keyed by name).
	let editingName = $state<string | null>(null);
	let editBaseUrl = $state('');
	let editMaxConcurrent = $state(DEFAULT_MAX_CONCURRENT);
	let editKey = $state('');
	let savingEdit = $state(false);
	let editError = $state('');

	// --- Default-rule repoint state ---
	let savingDefault = $state(false);

	// --- Add model-rule form ---
	let ruleModel = $state('');
	let ruleCredential = $state('');
	let ruleProviderModel = $state('');
	let savingRule = $state(false);
	let ruleError = $state('');
	let deletingRuleId = $state<string | null>(null);

	const defaultRule = $derived(rules.find((r) => r.match_model === null) ?? null);
	const modelRules = $derived(rules.filter((r) => r.match_model !== null));
	const credentialNames = $derived(credentials.map((c) => c.name));

	const proxyBaseUrl = $derived(
		typeof window === 'undefined' ? '' : `${window.location.origin}/proxy/anthropic`
	);
	let copied = $state(false);

	$effect(() => {
		loadAll();
	});

	async function loadAll() {
		loading = true;
		error = '';
		try {
			const [creds, routing] = await Promise.all([
				api.get<Credential[]>('/api/v1/me/credentials'),
				api.get<Rule[]>('/api/v1/me/proxy-routing')
			]);
			credentials = creds;
			rules = routing;
			// Default the add-rule credential picker to the first credential.
			if (!ruleCredential && credentials.length > 0) {
				ruleCredential = credentials[0].name;
			}
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load proxy configuration';
		} finally {
			loading = false;
		}
	}

	function clearBanners() {
		error = '';
		success = '';
	}

	// --- Credentials ---

	async function handleAddCredential(event: SubmitEvent) {
		event.preventDefault();
		credError = '';
		clearBanners();

		const name = credName.trim();
		const baseUrl = credBaseUrl.trim();
		const key = credKey.trim();

		if (!name) {
			credError = 'Name is required.';
			return;
		}
		if (!key) {
			// A key is required to create or rotate a credential. (Cap-only
			// updates go through the existing credential's own controls, but
			// this form is the create/rotate entry point.)
			credError = 'API key is required.';
			return;
		}
		if (!key.startsWith(KEY_PREFIX)) {
			credError = `API key must start with "${KEY_PREFIX}".`;
			return;
		}
		if (key.length > KEY_MAX_LEN) {
			credError = `API key must be at most ${KEY_MAX_LEN} characters.`;
			return;
		}
		if (baseUrl && !baseUrl.startsWith('https://')) {
			credError = 'Base URL must be an https:// URL.';
			return;
		}
		if (
			!Number.isInteger(credMaxConcurrent) ||
			credMaxConcurrent < MIN_MAX_CONCURRENT ||
			credMaxConcurrent > MAX_MAX_CONCURRENT
		) {
			credError = `Max concurrent must be a whole number between ${MIN_MAX_CONCURRENT} and ${MAX_MAX_CONCURRENT}.`;
			return;
		}

		const body: { key: string; base_url?: string; max_concurrent: number } = {
			key,
			max_concurrent: credMaxConcurrent
		};
		if (baseUrl) body.base_url = baseUrl;

		savingCred = true;
		try {
			await api.put<void>(`/api/v1/me/credentials/${encodeURIComponent(name)}`, body);
			success = `Credential "${name}" saved.`;
			credName = '';
			credKey = '';
			credBaseUrl = DEFAULT_BASE_URL;
			credMaxConcurrent = DEFAULT_MAX_CONCURRENT;
			await loadAll();
		} catch (err) {
			credError = err instanceof Error ? err.message : 'Failed to save credential';
		} finally {
			savingCred = false;
		}
	}

	async function handleDeleteCredential(name: string) {
		deletingCred = true;
		clearBanners();
		try {
			await api.delete<void>(`/api/v1/me/credentials/${encodeURIComponent(name)}`);
			confirmingDelete = null;
			success = `Credential "${name}" removed.`;
			await loadAll();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to remove credential';
		} finally {
			deletingCred = false;
		}
	}

	function openEdit(cred: Credential) {
		editingName = cred.name;
		editBaseUrl = cred.base_url;
		editMaxConcurrent = cred.max_concurrent;
		editKey = '';
		editError = '';
		confirmingDelete = null;
	}

	function cancelEdit() {
		editingName = null;
		editKey = '';
		editError = '';
	}

	async function handleEditCredential(event: SubmitEvent) {
		event.preventDefault();
		editError = '';
		clearBanners();

		const name = editingName;
		if (!name) return;

		const baseUrl = editBaseUrl.trim();
		const key = editKey.trim();

		if (!baseUrl.startsWith('https://')) {
			editError = 'Base URL must be an https:// URL.';
			return;
		}
		if (
			!Number.isInteger(editMaxConcurrent) ||
			editMaxConcurrent < MIN_MAX_CONCURRENT ||
			editMaxConcurrent > MAX_MAX_CONCURRENT
		) {
			editError = `Max concurrent must be a whole number between ${MIN_MAX_CONCURRENT} and ${MAX_MAX_CONCURRENT}.`;
			return;
		}
		if (key) {
			if (!key.startsWith(KEY_PREFIX)) {
				editError = `API key must start with "${KEY_PREFIX}".`;
				return;
			}
			if (key.length > KEY_MAX_LEN) {
				editError = `API key must be at most ${KEY_MAX_LEN} characters.`;
				return;
			}
		}

		const body: { base_url: string; max_concurrent: number; key?: string } = {
			base_url: baseUrl,
			max_concurrent: editMaxConcurrent
		};
		if (key) body.key = key;

		savingEdit = true;
		try {
			await api.put<void>(`/api/v1/me/credentials/${encodeURIComponent(name)}`, body);
			success = `Credential "${name}" updated.`;
			cancelEdit();
			await loadAll();
		} catch (err) {
			editError = err instanceof Error ? err.message : 'Failed to update credential';
		} finally {
			savingEdit = false;
		}
	}

	// --- Routing ---

	async function repointDefault(credentialName: string) {
		if (!credentialName || credentialName === defaultRule?.credential_name) return;
		savingDefault = true;
		clearBanners();
		try {
			await api.put<void>('/api/v1/me/proxy-routing', {
				match_model: null,
				credential_name: credentialName
			});
			success = 'Default route updated.';
			await loadAll();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to update default route';
		} finally {
			savingDefault = false;
		}
	}

	async function handleAddRule(event: SubmitEvent) {
		event.preventDefault();
		ruleError = '';
		clearBanners();

		const model = ruleModel.trim();
		const credential = ruleCredential;
		const providerModel = ruleProviderModel.trim();

		if (!model) {
			ruleError = 'Model is required.';
			return;
		}
		if (!credential) {
			ruleError = 'Pick a credential to route to.';
			return;
		}

		savingRule = true;
		try {
			await api.put<void>('/api/v1/me/proxy-routing', {
				match_model: model,
				credential_name: credential,
				provider_model: providerModel || null
			});
			success = `Rule for "${model}" saved.`;
			ruleModel = '';
			ruleProviderModel = '';
			await loadAll();
		} catch (err) {
			ruleError = err instanceof Error ? err.message : 'Failed to save routing rule';
		} finally {
			savingRule = false;
		}
	}

	async function handleDeleteRule(id: string) {
		deletingRuleId = id;
		clearBanners();
		try {
			await api.delete<void>(`/api/v1/me/proxy-routing/${id}`);
			success = 'Routing rule removed.';
			await loadAll();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to remove routing rule';
		} finally {
			deletingRuleId = null;
		}
	}

	async function copyProxyUrl() {
		if (!proxyBaseUrl) return;
		try {
			await navigator.clipboard.writeText(proxyBaseUrl);
			copied = true;
			setTimeout(() => (copied = false), 1500);
		} catch (err) {
			// Clipboard API can fail in some contexts (unfocused page, insecure
			// iframe). Log but don't surface — the user can copy manually.
			console.warn('Failed to copy proxy URL to clipboard:', err);
		}
	}

	function formatTimestamp(ts: string): string {
		if (!ts) return '';
		try {
			return new Date(ts).toLocaleString();
		} catch {
			return ts;
		}
	}
</script>

<svelte:head>
	<title>Proxy - TraceVault</title>
</svelte:head>

<div class="space-y-6">
	<div>
		<h1 class="text-2xl font-bold">LLM Proxy</h1>
		<p class="text-muted-foreground mt-1 text-sm">
			Route AI coding tools (Claude Code, GSD2, Cursor, Codex CLI) through TraceVault by
			pointing them at the proxy URL below. Store one or more named credentials and route
			individual models to them. Stored keys are used internally — they are never returned to
			the browser after saving.
		</p>
	</div>

	{#if error}
		<ErrorState message={error} />
	{/if}

	{#if success}
		<Alert.Root>
			<Alert.Title>Success</Alert.Title>
			<Alert.Description>{success}</Alert.Description>
		</Alert.Root>
	{/if}

	{#if loading}
		<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm">
			<span
				class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"
			></span>
			Loading...
		</div>
	{:else}
		<!-- ============================ Credentials ============================ -->
		<div class="border-border overflow-hidden rounded-lg border">
			<div class="bg-muted/30 px-4 py-3">
				<span class="text-sm font-semibold">Credentials</span>
				<p class="text-muted-foreground mt-0.5 text-xs">
					Named upstream credentials the proxy authenticates with on your behalf.
				</p>
			</div>
			<div class="space-y-4 p-4">
				{#if credentials.length === 0}
					<p class="text-muted-foreground text-sm">
						No credentials yet. Add one below — your first credential becomes the default
						route for all models.
					</p>
				{:else}
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head class="text-xs">Name</Table.Head>
								<Table.Head class="text-xs">Base URL</Table.Head>
								<Table.Head class="text-xs">Cap</Table.Head>
								<Table.Head class="text-xs">Added</Table.Head>
								<Table.Head class="text-xs"></Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#each credentials as cred (cred.name)}
								<Table.Row class="hover:bg-muted/40 transition-colors">
									<Table.Cell class="text-xs font-medium">{cred.name}</Table.Cell>
									<Table.Cell class="font-mono text-xs">{cred.base_url}</Table.Cell>
									<Table.Cell class="text-xs">{cred.max_concurrent}</Table.Cell>
									<Table.Cell class="text-muted-foreground text-xs">
										{formatTimestamp(cred.configured_at)}
									</Table.Cell>
									<Table.Cell class="text-xs">
										<div class="flex justify-end gap-2">
											{#if confirmingDelete === cred.name}
												<Button
													variant="destructive"
													size="sm"
													disabled={deletingCred}
													onclick={() => handleDeleteCredential(cred.name)}
												>
													{deletingCred ? 'Removing...' : 'Confirm'}
												</Button>
												<Button
													variant="ghost"
													size="sm"
													disabled={deletingCred}
													onclick={() => (confirmingDelete = null)}
												>
													Cancel
												</Button>
											{:else}
												<Button
													variant="outline"
													size="sm"
													onclick={() => openEdit(cred)}
												>
													Edit
												</Button>
												<Button
													variant="outline"
													size="sm"
													onclick={() => {
														confirmingDelete = cred.name;
														editingName = null;
													}}
												>
													Delete
												</Button>
											{/if}
										</div>
									</Table.Cell>
								</Table.Row>
								{#if editingName === cred.name}
									<Table.Row>
										<Table.Cell colspan={5} class="bg-muted/20 px-4 py-3">
											<form onsubmit={handleEditCredential} class="grid max-w-lg gap-3">
												<p class="text-sm font-medium">Edit "{cred.name}"</p>
												{#if editError}
													<p class="text-destructive text-sm">{editError}</p>
												{/if}
												<div class="grid gap-2">
													<Label>Name</Label>
													<p class="text-muted-foreground text-xs font-medium">{cred.name}</p>
												</div>
												<div class="grid gap-2">
													<Label for="edit_base_url">Base URL</Label>
													<Input
														id="edit_base_url"
														bind:value={editBaseUrl}
														placeholder={DEFAULT_BASE_URL}
														autocomplete="off"
													/>
												</div>
												<div class="grid gap-2">
													<Label for="edit_cap">Max concurrent requests</Label>
													<Input
														id="edit_cap"
														type="number"
														min={MIN_MAX_CONCURRENT}
														max={MAX_MAX_CONCURRENT}
														step={1}
														bind:value={editMaxConcurrent}
														class="max-w-[8rem]"
													/>
													<p class="text-muted-foreground text-xs">
														Range {MIN_MAX_CONCURRENT}–{MAX_MAX_CONCURRENT}.
													</p>
												</div>
												<div class="grid gap-2">
													<Label for="edit_key">New key (leave blank to keep current key)</Label>
													<Input
														id="edit_key"
														type="password"
														autocomplete="off"
														bind:value={editKey}
														placeholder="sk-ant-..."
													/>
													<p class="text-muted-foreground text-xs">
														The existing key is never shown. Enter a new key only to rotate it.
													</p>
												</div>
												<div class="flex gap-2">
													<Button type="submit" disabled={savingEdit}>
														{savingEdit ? 'Saving...' : 'Save'}
													</Button>
													<Button
														type="button"
														variant="ghost"
														disabled={savingEdit}
														onclick={cancelEdit}
													>
														Cancel
													</Button>
												</div>
											</form>
										</Table.Cell>
									</Table.Row>
								{/if}
							{/each}
						</Table.Body>
					</Table.Root>
					<p class="text-muted-foreground text-xs">
						Deleting a credential also removes any routing rules pointing at it.
					</p>
				{/if}

				<form onsubmit={handleAddCredential} class="grid max-w-lg gap-3 border-t pt-4">
					<p class="text-sm font-medium">Add / update credential</p>
					{#if credError}
						<p class="text-destructive text-sm">{credError}</p>
					{/if}
					<div class="grid gap-2">
						<Label for="cred_name">Name</Label>
						<Input
							id="cred_name"
							bind:value={credName}
							placeholder="e.g., default, fast"
							autocomplete="off"
						/>
						<p class="text-muted-foreground text-xs">
							Reusing an existing name rotates that credential's key.
						</p>
					</div>
					<div class="grid gap-2">
						<Label for="cred_base_url">Base URL</Label>
						<Input
							id="cred_base_url"
							bind:value={credBaseUrl}
							placeholder={DEFAULT_BASE_URL}
							autocomplete="off"
						/>
					</div>
					<div class="grid gap-2">
						<Label for="cred_key">API key</Label>
						<Input
							id="cred_key"
							type="password"
							autocomplete="off"
							bind:value={credKey}
							placeholder="sk-ant-..."
						/>
						<p class="text-muted-foreground text-xs">
							Saved keys are never displayed again.
						</p>
					</div>
					<div class="grid gap-2">
						<Label for="cred_cap">Max concurrent requests</Label>
						<Input
							id="cred_cap"
							type="number"
							min={MIN_MAX_CONCURRENT}
							max={MAX_MAX_CONCURRENT}
							step={1}
							bind:value={credMaxConcurrent}
							class="max-w-[8rem]"
						/>
						<p class="text-muted-foreground text-xs">
							Range {MIN_MAX_CONCURRENT}–{MAX_MAX_CONCURRENT}; default {DEFAULT_MAX_CONCURRENT}.
						</p>
					</div>
					<div>
						<Button type="submit" disabled={savingCred}>
							{savingCred ? 'Saving...' : 'Save credential'}
						</Button>
					</div>
				</form>
			</div>
		</div>

		<!-- ============================== Routing ============================== -->
		<div class="border-border overflow-hidden rounded-lg border">
			<div class="bg-muted/30 px-4 py-3">
				<span class="text-sm font-semibold">Routing</span>
				<p class="text-muted-foreground mt-0.5 text-xs">
					Route specific request models to a credential, optionally rewriting the model sent
					upstream. All other models use the default route.
				</p>
			</div>
			<div class="space-y-4 p-4">
				{#if credentials.length === 0}
					<p class="text-muted-foreground text-sm">
						Add a credential first — routing rules point at a named credential.
					</p>
				{:else}
					<Table.Root>
						<Table.Header>
							<Table.Row>
								<Table.Head class="text-xs">Model</Table.Head>
								<Table.Head class="text-xs">→ Credential</Table.Head>
								<Table.Head class="text-xs">Provider model</Table.Head>
								<Table.Head class="text-xs"></Table.Head>
							</Table.Row>
						</Table.Header>
						<Table.Body>
							{#if defaultRule}
								<Table.Row class="hover:bg-muted/40 transition-colors">
									<Table.Cell class="text-xs font-medium">
										Default — all other models
									</Table.Cell>
									<Table.Cell class="text-xs">
										<Select.Root
											type="single"
											value={defaultRule.credential_name}
											onValueChange={(v) => {
												if (v) repointDefault(v);
											}}
											disabled={savingDefault}
										>
											<Select.Trigger class="h-8 w-[12rem] text-xs">
												{defaultRule.credential_name}
											</Select.Trigger>
											<Select.Content>
												{#each credentialNames as name (name)}
													<Select.Item value={name}>{name}</Select.Item>
												{/each}
											</Select.Content>
										</Select.Root>
									</Table.Cell>
									<Table.Cell class="text-muted-foreground text-xs">—</Table.Cell>
									<Table.Cell class="text-xs"></Table.Cell>
								</Table.Row>
							{/if}
							{#each modelRules as rule (rule.id)}
								<Table.Row class="hover:bg-muted/40 transition-colors">
									<Table.Cell class="font-mono text-xs">{rule.match_model}</Table.Cell>
									<Table.Cell class="text-xs">{rule.credential_name}</Table.Cell>
									<Table.Cell class="font-mono text-xs">
										{rule.provider_model ?? '—'}
									</Table.Cell>
									<Table.Cell class="text-xs">
										<div class="flex justify-end">
											<Button
												variant="outline"
												size="sm"
												disabled={deletingRuleId === rule.id}
												onclick={() => handleDeleteRule(rule.id)}
											>
												{deletingRuleId === rule.id ? 'Removing...' : 'Delete'}
											</Button>
										</div>
									</Table.Cell>
								</Table.Row>
							{/each}
						</Table.Body>
					</Table.Root>

					<form onsubmit={handleAddRule} class="grid max-w-lg gap-3 border-t pt-4">
						<p class="text-sm font-medium">Add model rule</p>
						{#if ruleError}
							<p class="text-destructive text-sm">{ruleError}</p>
						{/if}
						<div class="grid gap-2">
							<Label for="rule_model">Model</Label>
							<Input
								id="rule_model"
								bind:value={ruleModel}
								placeholder="e.g., claude-haiku"
								autocomplete="off"
							/>
						</div>
						<div class="grid gap-2">
							<Label>Credential</Label>
							<Select.Root
								type="single"
								value={ruleCredential}
								onValueChange={(v) => {
									if (v) ruleCredential = v;
								}}
							>
								<Select.Trigger class="w-[12rem] text-sm">
									{ruleCredential || 'Select credential'}
								</Select.Trigger>
								<Select.Content>
									{#each credentialNames as name (name)}
										<Select.Item value={name}>{name}</Select.Item>
									{/each}
								</Select.Content>
							</Select.Root>
						</div>
						<div class="grid gap-2">
							<Label for="rule_provider_model">Provider model (optional)</Label>
							<Input
								id="rule_provider_model"
								bind:value={ruleProviderModel}
								placeholder="e.g., claude-3-5-haiku-latest"
								autocomplete="off"
							/>
							<p class="text-muted-foreground text-xs">
								Rewrites the model sent upstream. Leave blank to forward the requested
								model as-is.
							</p>
						</div>
						<div>
							<Button type="submit" disabled={savingRule}>
								{savingRule ? 'Saving...' : 'Add rule'}
							</Button>
						</div>
					</form>
				{/if}
			</div>
		</div>

		<!-- =============================== Setup =============================== -->
		<div class="border-border max-w-lg overflow-hidden rounded-lg border">
			<div class="bg-muted/30 px-4 py-3">
				<span class="text-sm font-semibold">How to use</span>
				<p class="text-muted-foreground mt-0.5 text-xs">
					Configure your tool to send Anthropic requests through TraceVault.
				</p>
			</div>
			<div class="space-y-4 p-4 text-sm">
				<div>
					<Label class="mb-1 block">Proxy base URL</Label>
					<div class="flex items-center gap-2">
						<code class="bg-muted flex-1 truncate rounded px-2 py-1 text-xs">
							{proxyBaseUrl || '(loading…)'}
						</code>
						<Button
							type="button"
							variant="outline"
							size="sm"
							onclick={copyProxyUrl}
							disabled={!proxyBaseUrl}
						>
							{copied ? 'Copied' : 'Copy'}
						</Button>
					</div>
				</div>
				<div class="text-muted-foreground space-y-1 text-xs leading-relaxed">
					<p>Set these environment variables for your AI tool:</p>
					<pre
						class="bg-muted overflow-x-auto rounded p-2 text-xs"><code
							>ANTHROPIC_BASE_URL={proxyBaseUrl}
ANTHROPIC_API_KEY=&lt;your TraceVault session token&gt;</code
						></pre>
					<p>
						Your TraceVault session token is in
						<code>~/.tracevault/credentials.json</code> after running
						<code>tracevault login</code>, or run <code>tracevault proxy info</code> for the
						full configuration snippet.
					</p>
				</div>
			</div>
		</div>
	{/if}
</div>
