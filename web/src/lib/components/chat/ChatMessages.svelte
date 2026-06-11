<script lang="ts">
	import type { ChatMessage, MentionItem } from '$lib/types';
	import { MessageCircle, Bot, User } from '@lucide/svelte';
	import { marked } from 'marked';
	import { formatTime } from '$lib/utils/date';

	let {
		messages,
		loading,
		sending,
		slug,
		mentions = []
	}: {
		messages: ChatMessage[];
		loading: boolean;
		sending: boolean;
		slug: string;
		mentions?: MentionItem[];
	} = $props();

	let container: HTMLDivElement | undefined = $state();

	$effect(() => {
		messages.length;
		sending;
		if (container) {
			setTimeout(() => container?.scrollTo({ top: container.scrollHeight, behavior: 'smooth' }), 0);
		}
	});

	function mentionPillClass(type: string): string {
		switch (type) {
			case 'user':
				return 'bg-primary/15 text-primary';
			case 'repo':
				return 'bg-emerald-500/15 text-emerald-600';
			case 'model':
				return 'bg-amber-500/15 text-amber-600';
			default:
				return 'bg-muted text-foreground';
		}
	}

	function renderContent(content: string, orgSlug: string, mentionItems: MentionItem[]): string {
		let html = marked.parse(content) as string;
		// Replace [Session #id] with links
		html = html.replace(
			/\[Session #([^\]]+)\]/g,
			(_match, id) =>
				`<a href="/orgs/${orgSlug}/traces/sessions/${id}" class="inline-flex items-center gap-0.5 rounded bg-primary/10 px-1.5 py-0.5 text-xs font-medium text-primary transition-colors hover:bg-primary/20">[Session #${id}]</a>`
		);
		// Replace @mentions with colored pills
		if (mentionItems.length > 0) {
			const sorted = [...mentionItems].sort((a, b) => b.display.length - a.display.length);
			for (const m of sorted) {
				const escaped = m.display.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
				const regex = new RegExp(`@${escaped}(?![\\w.-])`, 'g');
				html = html.replace(
					regex,
					`<span class="inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium ${mentionPillClass(m.type)}">@${m.display}</span>`
				);
			}
		}
		return html;
	}
</script>

<div bind:this={container} class="flex-1 overflow-y-auto">
	{#if loading}
		<div class="flex items-center justify-center py-20">
			<span class="inline-block h-5 w-5 animate-spin rounded-full border-2 border-primary border-t-transparent"></span>
		</div>
	{:else if messages.length === 0 && !sending}
		<div class="flex h-full flex-col items-center justify-center gap-4 px-4">
			<div class="flex h-14 w-14 items-center justify-center rounded-2xl bg-primary/10">
				<MessageCircle class="h-7 w-7 text-primary" />
			</div>
			<div class="text-center">
				<h3 class="text-lg font-semibold text-foreground">Ask about your sessions</h3>
				<p class="mt-1 max-w-sm text-sm text-muted-foreground">
					Search across transcripts, find what your team worked on, or explore session history using natural language.
				</p>
			</div>
			<div class="mt-2 flex flex-wrap justify-center gap-2">
				{#each ['What did the team work on today?', 'Show recent auth changes', 'Who worked on the API last week?'] as suggestion}
					<span class="rounded-full border border-border bg-muted/50 px-3 py-1.5 text-xs text-muted-foreground">
						{suggestion}
					</span>
				{/each}
			</div>
		</div>
	{:else}
		<div class="mx-auto max-w-3xl space-y-1 px-4 py-6">
			{#each messages as msg (msg.id)}
				<div class="flex gap-3 rounded-xl px-4 py-4 {msg.role === 'user' ? '' : 'bg-muted/40'}">
					<div class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg {msg.role === 'user' ? 'bg-primary/10' : 'bg-foreground/5'}">
						{#if msg.role === 'user'}
							<User class="h-4 w-4 text-primary" />
						{:else}
							<Bot class="h-4 w-4 text-muted-foreground" />
						{/if}
					</div>
					<div class="min-w-0 flex-1">
						<div class="mb-1 flex items-center gap-2">
							<span class="text-xs font-medium {msg.role === 'user' ? 'text-primary' : 'text-muted-foreground'}">
								{msg.role === 'user' ? 'You' : 'TraceVault'}
							</span>
							<span class="text-[10px] text-muted-foreground/50">{formatTime(msg.created_at)}</span>
						</div>
						<div class="chat-markdown">
							{@html renderContent(msg.content, slug, mentions)}
						</div>
					</div>
				</div>
			{/each}

			{#if sending}
				<div class="flex gap-3 rounded-xl bg-muted/40 px-4 py-4">
					<div class="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-foreground/5">
						<Bot class="h-4 w-4 text-muted-foreground" />
					</div>
					<div class="flex items-center gap-2">
						<div class="flex gap-1">
							<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/40" style="animation-delay: 0ms"></span>
							<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/40" style="animation-delay: 150ms"></span>
							<span class="h-2 w-2 animate-bounce rounded-full bg-muted-foreground/40" style="animation-delay: 300ms"></span>
						</div>
						<span class="text-xs text-muted-foreground">Searching transcripts...</span>
					</div>
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	.chat-markdown {
		font-size: 0.875rem;
		line-height: 1.7;
		color: var(--foreground);
	}

	.chat-markdown :global(> :first-child) {
		margin-top: 0;
	}

	.chat-markdown :global(p) {
		margin-top: 0.5rem;
		margin-bottom: 0.5rem;
	}

	.chat-markdown :global(h1),
	.chat-markdown :global(h2),
	.chat-markdown :global(h3),
	.chat-markdown :global(h4) {
		font-weight: 600;
		margin-top: 1rem;
		margin-bottom: 0.375rem;
	}

	.chat-markdown :global(ul),
	.chat-markdown :global(ol) {
		margin-top: 0.5rem;
		margin-bottom: 0.5rem;
		padding-left: 1.5rem;
	}

	.chat-markdown :global(ul) {
		list-style-type: disc;
	}

	.chat-markdown :global(ol) {
		list-style-type: decimal;
	}

	.chat-markdown :global(li) {
		margin-top: 0.25rem;
		margin-bottom: 0.25rem;
	}

	.chat-markdown :global(li > ul),
	.chat-markdown :global(li > ol) {
		margin-top: 0.125rem;
		margin-bottom: 0.125rem;
	}

	.chat-markdown :global(strong) {
		font-weight: 600;
	}

	.chat-markdown :global(em) {
		font-style: italic;
	}

	.chat-markdown :global(code) {
		font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, monospace;
		font-size: 0.8em;
		background: var(--muted);
		padding: 0.125rem 0.375rem;
		border-radius: 0.25rem;
	}

	.chat-markdown :global(pre) {
		margin-top: 0.75rem;
		margin-bottom: 0.75rem;
		padding: 0.75rem 1rem;
		background: var(--muted);
		border-radius: 0.5rem;
		overflow-x: auto;
		line-height: 1.6;
	}

	.chat-markdown :global(pre code) {
		background: none;
		padding: 0;
		border-radius: 0;
		font-size: inherit;
	}

	.chat-markdown :global(blockquote) {
		margin-top: 0.75rem;
		margin-bottom: 0.75rem;
		padding-left: 1rem;
		border-left: 3px solid var(--border);
		color: var(--muted-foreground);
		font-style: italic;
	}

	.chat-markdown :global(a) {
		color: var(--primary);
		text-decoration: underline;
		text-underline-offset: 2px;
	}

	.chat-markdown :global(a:hover) {
		opacity: 0.8;
	}
</style>
