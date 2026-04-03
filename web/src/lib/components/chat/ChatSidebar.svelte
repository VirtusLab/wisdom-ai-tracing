<script lang="ts">
	import type { Conversation } from '$lib/types';
	import Plus from '@lucide/svelte/icons/plus';
	import Trash2 from '@lucide/svelte/icons/trash-2';

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
		const now = Date.now();
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

<div class="flex h-full w-64 shrink-0 flex-col border-r border-border bg-sidebar">
	<div class="p-3">
		<button
			onclick={onCreate}
			class="flex w-full items-center gap-2 rounded-md border border-border px-3 py-2 text-sm font-medium transition-colors hover:bg-sidebar-accent"
		>
			<Plus class="h-4 w-4" />
			New Chat
		</button>
	</div>

	<div class="flex-1 overflow-y-auto px-2 pb-3">
		{#each grouped as group}
			<div class="mb-2">
				<p class="px-2 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
					{group.label}
				</p>
				{#each group.items as conv}
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						onclick={() => onSelect(conv.id)}
						class="group flex w-full cursor-pointer items-center justify-between rounded-md px-2 py-1.5 text-left text-sm transition-colors
							{activeConversationId === conv.id
							? 'bg-primary text-primary-foreground'
							: 'text-sidebar-foreground hover:bg-sidebar-accent'}"
					>
						<span class="truncate">{conv.title ?? 'Untitled'}</span>
						<button
							onclick={(e) => {
								e.stopPropagation();
								onDelete(conv.id);
							}}
							class="shrink-0 opacity-0 transition-opacity group-hover:opacity-100
								{activeConversationId === conv.id ? 'text-primary-foreground/70 hover:text-primary-foreground' : 'text-muted-foreground hover:text-foreground'}"
						>
							<Trash2 class="h-3.5 w-3.5" />
						</button>
					</div>
				{/each}
			</div>
		{/each}
	</div>
</div>
