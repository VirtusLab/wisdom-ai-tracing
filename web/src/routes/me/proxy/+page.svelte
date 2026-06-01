<script lang="ts">
	import { api } from '$lib/api';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Alert from '$lib/components/ui/alert/index.js';
	import ErrorState from '$lib/components/ErrorState.svelte';

	interface AnthropicKeyStatus {
		configured: boolean;
		configured_at: string | null;
		max_concurrent: number | null;
	}

	const DEFAULT_MAX_CONCURRENT = 8;
	const MIN_MAX_CONCURRENT = 1;
	const MAX_MAX_CONCURRENT = 256;

	let status: AnthropicKeyStatus | null = $state(null);
	let loading = $state(true);
	let saving = $state(false);
	let removing = $state(false);
	let confirmingRemove = $state(false);
	let error = $state('');
	let success = $state('');
	let newKey = $state('');
	let newMaxConcurrent: number = $state(DEFAULT_MAX_CONCURRENT);
	let copied = $state(false);

	const proxyBaseUrl = $derived(
		typeof window === 'undefined' ? '' : `${window.location.origin}/proxy/anthropic`
	);

	$effect(() => {
		loadStatus();
	});

	async function loadStatus() {
		loading = true;
		error = '';
		try {
			status = await api.get<AnthropicKeyStatus>('/api/v1/me/anthropic-key');
			// Pre-fill the form's cap with whatever's currently stored so a
			// "just rotate the key" flow doesn't accidentally reset the cap.
			if (status?.max_concurrent != null) {
				newMaxConcurrent = status.max_concurrent;
			}
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load proxy configuration';
		} finally {
			loading = false;
		}
	}

	/// True when the user has either typed a new key or changed the cap
	/// away from what's stored. The submit button is gated on this — it
	/// prevents the user from submitting a no-op request.
	function computeHasUnsavedChange() {
		const keyTyped = newKey.trim().length > 0;
		const capChanged =
			status?.max_concurrent != null && newMaxConcurrent !== status.max_concurrent;
		// When not configured yet, only a key counts — the cap on its own
		// can't be the first write (server returns 400 in that case).
		if (!status?.configured) return keyTyped;
		return keyTyped || capChanged;
	}

	const hasUnsavedChange = $derived.by(computeHasUnsavedChange);

	async function handleSave(event: SubmitEvent) {
		event.preventDefault();
		if (!hasUnsavedChange) return;
		// Defensive client-side bounds check. The server enforces the same
		// range (DB CHECK + handler validation) but failing here gives a
		// clearer error than a 400 from the API.
		if (
			!Number.isInteger(newMaxConcurrent) ||
			newMaxConcurrent < MIN_MAX_CONCURRENT ||
			newMaxConcurrent > MAX_MAX_CONCURRENT
		) {
			error = `Max concurrent must be a whole number between ${MIN_MAX_CONCURRENT} and ${MAX_MAX_CONCURRENT}.`;
			return;
		}

		// Build a minimal request body: only include `key` when the user
		// actually typed one. Cap is always sent so the server picks up
		// any change.
		const body: { key?: string; max_concurrent: number } = {
			max_concurrent: newMaxConcurrent
		};
		const trimmedKey = newKey.trim();
		if (trimmedKey.length > 0) body.key = trimmedKey;

		saving = true;
		error = '';
		success = '';
		try {
			await api.put<void>('/api/v1/me/anthropic-key', body);
			newKey = '';
			success = trimmedKey
				? 'Anthropic API key saved.'
				: 'Concurrency cap updated.';
			await loadStatus();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to save settings';
		} finally {
			saving = false;
		}
	}

	async function handleRemove() {
		removing = true;
		error = '';
		success = '';
		try {
			await api.delete<void>('/api/v1/me/anthropic-key');
			confirmingRemove = false;
			success = 'Anthropic API key removed.';
			await loadStatus();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to remove key';
		} finally {
			removing = false;
		}
	}

	async function copyProxyUrl() {
		if (!proxyBaseUrl) return;
		try {
			await navigator.clipboard.writeText(proxyBaseUrl);
			copied = true;
			setTimeout(() => (copied = false), 1500);
		} catch (err) {
			// Clipboard API can fail in some browsers / contexts (e.g. when the
			// page is not focused, or in non-secure-context iframes). Log so
			// it's debuggable but don't surface as a page-level error — the
			// user can still copy manually.
			console.warn('Failed to copy proxy URL to clipboard:', err);
		}
	}

	function formatTimestamp(ts: string | null): string {
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
			pointing them at the proxy URL below. Your stored Anthropic API key is used internally
			— it is never returned to the browser after saving.
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
		<!-- Anthropic API key configuration -->
		<div class="border-border max-w-lg overflow-hidden rounded-lg border">
			<div class="bg-muted/30 px-4 py-3">
				<span class="text-sm font-semibold">Anthropic API Key</span>
				<p class="text-muted-foreground mt-0.5 text-xs">
					Used by the proxy to authenticate with api.anthropic.com on your behalf.
				</p>
			</div>
			<div class="space-y-4 p-4">
				{#if status?.configured}
					<div class="flex items-center gap-2 text-sm">
						<span
							class="inline-block h-2 w-2 rounded-full bg-emerald-500"
							aria-hidden="true"
						></span>
						<span class="font-medium">Configured</span>
						{#if status.configured_at}
							<span class="text-muted-foreground"
								>last set {formatTimestamp(status.configured_at)}</span
							>
						{/if}
						{#if status.max_concurrent != null}
							<span class="text-muted-foreground" title="Max concurrent proxy requests">
								&middot; cap {status.max_concurrent}
							</span>
						{/if}
					</div>
				{:else}
					<div class="flex items-center gap-2 text-sm">
						<span
							class="bg-muted-foreground inline-block h-2 w-2 rounded-full"
							aria-hidden="true"
						></span>
						<span class="text-muted-foreground">Not configured</span>
					</div>
				{/if}

				<form onsubmit={handleSave} class="space-y-3">
					<div class="grid gap-2">
						<Label for="anthropic_key">
							{status?.configured ? 'Rotate key (optional)' : 'Set key'}
						</Label>
						<Input
							id="anthropic_key"
							type="password"
							autocomplete="off"
							bind:value={newKey}
							placeholder="sk-ant-..."
						/>
						<p class="text-muted-foreground text-xs">
							Saved keys are never displayed again. Get one from
							<a
								href="https://console.anthropic.com/settings/keys"
								target="_blank"
								rel="noreferrer"
								class="underline">console.anthropic.com</a
							>.
						</p>
					</div>

					<div class="grid gap-2">
						<Label for="max_concurrent">Max concurrent requests</Label>
						<Input
							id="max_concurrent"
							type="number"
							min={MIN_MAX_CONCURRENT}
							max={MAX_MAX_CONCURRENT}
							step={1}
							bind:value={newMaxConcurrent}
							class="max-w-[8rem]"
						/>
						<p class="text-muted-foreground text-xs">
							The proxy rejects further requests for this credential once this many are in
							flight. Range {MIN_MAX_CONCURRENT}–{MAX_MAX_CONCURRENT}; default {DEFAULT_MAX_CONCURRENT}.
							New value applies on the next proxy request; in-flight requests keep their
							existing budget.
						</p>
					</div>

					<div class="flex items-center gap-2">
						<Button type="submit" disabled={saving || !hasUnsavedChange}>
							{saving ? 'Saving...' : status?.configured ? 'Update' : 'Save'}
						</Button>
						{#if status?.configured}
							{#if confirmingRemove}
								<Button
									type="button"
									variant="destructive"
									disabled={removing}
									onclick={handleRemove}
								>
									{removing ? 'Removing...' : 'Confirm remove'}
								</Button>
								<Button
									type="button"
									variant="ghost"
									disabled={removing}
									onclick={() => (confirmingRemove = false)}
								>
									Cancel
								</Button>
							{:else}
								<Button
									type="button"
									variant="ghost"
									onclick={() => (confirmingRemove = true)}
								>
									Remove
								</Button>
							{/if}
						{/if}
					</div>
				</form>
			</div>
		</div>

		<!-- How-to / copy block -->
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
