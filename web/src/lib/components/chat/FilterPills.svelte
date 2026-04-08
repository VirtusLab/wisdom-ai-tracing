<script lang="ts">
	import type { ExtractedFilters } from '$lib/types';
	import { Filter } from '@lucide/svelte';

	let { filters }: { filters: ExtractedFilters } = $props();

	const pills = $derived.by(() => {
		const items: { label: string; value: string }[] = [];
		if (filters.user) items.push({ label: 'user', value: filters.user });
		if (filters.repo) items.push({ label: 'repo', value: filters.repo });
		if (filters.time_from || filters.time_to) {
			const from = filters.time_from ?? '...';
			const to = filters.time_to ?? '...';
			items.push({ label: 'time', value: `${from} \u2192 ${to}` });
		}
		if (filters.model) items.push({ label: 'model', value: filters.model });
		return items;
	});
</script>

{#if pills.length > 0}
	<div class="flex items-center gap-1.5 border-b border-border bg-muted/30 px-4 py-2">
		<Filter class="h-3 w-3 text-muted-foreground/60" />
		{#each pills as pill}
			<span class="inline-flex items-center gap-1 rounded-md border border-border bg-background px-2 py-0.5 text-[11px]">
				<span class="text-muted-foreground">{pill.label}:</span>
				<span class="font-medium text-foreground">{pill.value}</span>
			</span>
		{/each}
	</div>
{/if}
