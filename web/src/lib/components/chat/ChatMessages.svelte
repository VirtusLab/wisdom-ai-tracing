<script lang="ts">
	import type { ChatMessage } from '$lib/types';

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
		// Track messages length to scroll on new messages
		messages.length;
		sending;
		if (container) {
			// Use setTimeout to ensure DOM has updated
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
</script>

<div bind:this={container} class="flex-1 overflow-y-auto p-4 space-y-4">
	{#if loading}
		<div class="flex items-center justify-center py-12">
			<span class="inline-block h-5 w-5 animate-spin rounded-full border-2 border-current border-t-transparent text-muted-foreground"></span>
		</div>
	{:else if messages.length === 0 && !sending}
		<div class="flex h-full items-center justify-center">
			<p class="text-muted-foreground text-sm">Start a conversation by typing a message below.</p>
		</div>
	{:else}
		{#each messages as msg (msg.id)}
			{#if msg.role === 'user'}
				<div class="flex justify-end">
					<div class="max-w-[70%] rounded-lg bg-primary px-4 py-2 text-primary-foreground">
						<p class="whitespace-pre-wrap text-sm">{msg.content}</p>
					</div>
				</div>
			{:else}
				<div class="flex justify-start">
					<div class="max-w-[70%] rounded-lg bg-muted px-4 py-2">
						<p class="whitespace-pre-wrap text-sm">
							{#each parseContent(msg.content) as part}
								{#if part.sessionId}
									<a
										href="/orgs/{slug}/traces/sessions/{part.sessionId}"
										class="text-primary underline hover:text-primary/80"
									>{part.text}</a>
								{:else}
									{part.text}
								{/if}
							{/each}
						</p>
					</div>
				</div>
			{/if}
		{/each}

		{#if sending}
			<div class="flex justify-start">
				<div class="flex items-center gap-2 rounded-lg bg-muted px-4 py-2">
					<span class="inline-block h-3 w-3 animate-spin rounded-full border-2 border-current border-t-transparent text-muted-foreground"></span>
					<span class="text-sm text-muted-foreground">Thinking...</span>
				</div>
			</div>
		{/if}
	{/if}
</div>
