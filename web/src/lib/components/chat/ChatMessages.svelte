<script lang="ts">
	import type { ChatMessage } from '$lib/types';
	import { MessageCircle, Bot, User } from '@lucide/svelte';

	let {
		messages,
		loading,
		sending,
		slug
	}: {
		messages: ChatMessage[];
		loading: boolean;
		sending: boolean;
		slug: string;
	} = $props();

	let container: HTMLDivElement | undefined = $state();

	$effect(() => {
		messages.length;
		sending;
		if (container) {
			setTimeout(() => container?.scrollTo({ top: container.scrollHeight, behavior: 'smooth' }), 0);
		}
	});

	function parseContent(content: string): { text: string; sessionId: string | null }[] {
		const parts: { text: string; sessionId: string | null }[] = [];
		const regex = /\[Session #([^\]]+)\]/g;
		let lastIndex = 0;
		let match;

		while ((match = regex.exec(content)) !== null) {
			if (match.index > lastIndex) {
				parts.push({ text: content.slice(lastIndex, match.index), sessionId: null });
			}
			parts.push({ text: match[0], sessionId: match[1] });
			lastIndex = regex.lastIndex;
		}

		if (lastIndex < content.length) {
			parts.push({ text: content.slice(lastIndex), sessionId: null });
		}

		return parts.length > 0 ? parts : [{ text: content, sessionId: null }];
	}

	function formatTime(iso: string): string {
		return new Date(iso).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
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
						<div class="whitespace-pre-wrap text-sm leading-relaxed text-foreground">
							{#each parseContent(msg.content) as part}
								{#if part.sessionId}
									<a
										href="/orgs/{slug}/traces/sessions/{part.sessionId}"
										class="inline-flex items-center gap-0.5 rounded bg-primary/10 px-1.5 py-0.5 text-xs font-medium text-primary transition-colors hover:bg-primary/20"
									>{part.text}</a>
								{:else}
									{part.text}
								{/if}
							{/each}
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
