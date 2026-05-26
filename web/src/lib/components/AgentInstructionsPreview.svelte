<script lang="ts">
	import { Button } from '$lib/components/ui/button/index.js';
	import { api } from '$lib/api';

	interface Props {
		slug: string;
		repoId: string;
	}

	let { slug, repoId }: Props = $props();

	let open = $state(false);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let content = $state<string | null>(null);
	let copyStatus = $state<'idle' | 'copied' | 'failed'>('idle');

	async function fetchContent() {
		loading = true;
		error = null;
		try {
			const resp = await api.get<{ format: string; content: string }>(
				`/api/v1/orgs/${slug}/repos/${repoId}/policies/agent-instructions`
			);
			content = resp.content;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to fetch instructions';
		} finally {
			loading = false;
		}
	}

	async function toggle() {
		if (open) {
			open = false;
			return;
		}
		if (content !== null) {
			open = true;
			return; // cached
		}
		open = true;
		await fetchContent();
	}

	async function copyToClipboard() {
		if (!content) return;
		try {
			await navigator.clipboard.writeText(content);
			copyStatus = 'copied';
			setTimeout(() => (copyStatus = 'idle'), 1500);
		} catch {
			copyStatus = 'failed';
			setTimeout(() => (copyStatus = 'idle'), 2500);
		}
	}
</script>

<div class="space-y-2">
	<div class="flex items-center justify-between">
		<h2 class="text-sm font-semibold">Agent instructions</h2>
		<div class="flex gap-2">
			{#if open && content}
				<Button variant="outline" size="sm" onclick={copyToClipboard}>
					{copyStatus === 'copied' ? 'Copied' : copyStatus === 'failed' ? 'Copy failed' : 'Copy'}
				</Button>
				<Button variant="outline" size="sm" onclick={fetchContent} disabled={loading}>
					Refresh
				</Button>
			{/if}
			<Button variant="outline" size="sm" onclick={toggle}>
				{open ? 'Hide' : 'Preview'}
			</Button>
		</div>
	</div>

	<p class="text-muted-foreground text-xs">
		Markdown instructions that agents see when they call <code>tracevault agent-policies</code>
		or the <code>agent_policies</code> MCP tool. Updates automatically when policies change.
	</p>

	{#if open}
		<div class="bg-muted/50 rounded-md border p-3">
			{#if loading}
				<div class="text-muted-foreground flex items-center gap-2 text-sm">
					<span
						class="inline-block h-3 w-3 animate-spin rounded-full border-2 border-current border-t-transparent"
					></span>
					Loading…
				</div>
			{:else if error}
				<p class="text-destructive text-sm">{error}</p>
			{:else if content}
				<pre
					class="text-foreground overflow-x-auto whitespace-pre-wrap font-mono text-xs leading-relaxed">{content}</pre>
			{/if}
		</div>
	{/if}
</div>
