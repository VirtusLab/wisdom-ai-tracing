<script lang="ts">
	import type { ExtractedFilters } from '$lib/types';

	let { filters }: { filters: ExtractedFilters } = $props();

	const pills = $derived.by(() => {
		const items: string[] = [];
		if (filters.user) items.push(`user: ${filters.user}`);
		if (filters.repo) items.push(`repo: ${filters.repo}`);
		if (filters.time_from || filters.time_to) {
			const from = filters.time_from ?? '...';
			const to = filters.time_to ?? '...';
			items.push(`${from} \u2192 ${to}`);
		}
		if (filters.model) items.push(`model: ${filters.model}`);
		return items;
	});
</script>

{#if pills.length > 0}
	<div class="flex flex-wrap gap-1.5 px-4 py-2">
		{#each pills as pill}
			<span class="rounded-full bg-muted px-3 py-1 text-xs text-muted-foreground">
				{pill}
			</span>
		{/each}
	</div>
{/if}
