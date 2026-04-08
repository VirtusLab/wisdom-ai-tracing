<script lang="ts">
	import type { Conversation } from '$lib/types';
	import { Plus, Trash2, MessageCircle } from '@lucide/svelte';

	let {
		conversations,
		activeConversationId,
		onSelect,
		onCreate,
		onDelete
	}: {
		conversations: Conversation[];
		activeConversationId: string | null;
		onSelect: (id: string) => void;
		onCreate: () => void;
		onDelete: (id: string) => void;
		onRename: (id: string, title: string) => void;
	} = $props();

	function groupByTime(items: Conversation[]) {
		const todayStart = new Date();
		todayStart.setHours(0, 0, 0, 0);
		const yesterdayStart = new Date(todayStart.getTime() - 86400000);
		const weekStart = new Date(todayStart.getTime() - 7 * 86400000);

		const groups: { label: string; items: Conversation[] }[] = [
			{ label: 'Today', items: [] },
			{ label: 'Yesterday', items: [] },
			{ label: 'Last 7 days', items: [] },
			{ label: 'Older', items: [] }
		];

		for (const c of items) {
			const t = new Date(c.updated_at).getTime();
			if (t >= todayStart.getTime()) groups[0].items.push(c);
			else if (t >= yesterdayStart.getTime()) groups[1].items.push(c);
			else if (t >= weekStart.getTime()) groups[2].items.push(c);
			else groups[3].items.push(c);
		}

		return groups.filter((g) => g.items.length > 0);
	}

	const grouped = $derived(groupByTime(conversations));
</script>

<div class="flex h-full w-72 shrink-0 flex-col border-r border-border bg-sidebar">
	<div class="flex items-center justify-between border-b border-border px-4 py-3" style="min-height: 60px;">
		<h2 class="text-sm font-semibold text-foreground">Conversations</h2>
		<button
			onclick={onCreate}
			class="flex h-8 w-8 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-sidebar-accent hover:text-foreground"
			title="New Chat"
		>
			<Plus class="h-4 w-4" />
		</button>
	</div>

	<div class="flex-1 overflow-y-auto">
		{#if conversations.length === 0}
			<div class="flex flex-col items-center justify-center gap-3 px-4 py-12 text-center">
				<div class="flex h-10 w-10 items-center justify-center rounded-full bg-muted">
					<MessageCircle class="h-5 w-5 text-muted-foreground" />
				</div>
				<div>
					<p class="text-sm font-medium text-foreground">No conversations yet</p>
					<p class="mt-0.5 text-xs text-muted-foreground">Start by asking a question</p>
				</div>
			</div>
		{:else}
			<div class="px-2 py-2 space-y-3">
				{#each grouped as group}
					<div>
						<p class="px-2 pb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/70">
							{group.label}
						</p>
						<div class="space-y-0.5">
							{#each group.items as conv}
								<button
									onclick={() => onSelect(conv.id)}
									class="group flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-sm transition-all
										{activeConversationId === conv.id
										? 'bg-primary/10 text-primary font-medium'
										: 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
								>
									<MessageCircle class="h-3.5 w-3.5 shrink-0 opacity-50" />
									<span class="flex-1 truncate">{conv.title ?? 'Untitled'}</span>
									<span
										role="button"
										tabindex="-1"
										onclick={(e) => {
											e.stopPropagation();
											onDelete(conv.id);
										}}
										onkeydown={(e) => {
											if (e.key === 'Enter' || e.key === ' ') {
												e.stopPropagation();
												onDelete(conv.id);
											}
										}}
										class="shrink-0 rounded p-0.5 opacity-0 transition-opacity group-hover:opacity-100 hover:bg-destructive/10 hover:text-destructive"
									>
										<Trash2 class="h-3 w-3" />
									</span>
								</button>
							{/each}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
