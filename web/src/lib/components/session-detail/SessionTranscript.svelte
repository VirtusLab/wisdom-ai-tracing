<script lang="ts">
	import { formatDateTime } from '$lib/utils/date';

	interface TranscriptRecordData {
		record_type: string;
		timestamp: string | null;
		content_types: string[];
		tool_name: string | null;
		text: string | null;
		usage: {
			input_tokens: number;
			output_tokens: number;
			cache_read_tokens: number;
			cache_write_tokens: number;
			cost_usd: number;
		} | null;
		model: string | null;
	}

	interface Props {
		records: TranscriptRecordData[];
	}

	let { records }: Props = $props();

	function fmtTime(ts: string | null): string {
		if (!ts) return '';
		try {
			return formatDateTime(ts);
		} catch {
			return '';
		}
	}

	const TYPE_COLORS: Record<string, string> = {
		user: '#3ecf8e',
		assistant: '#a78bfa',
		progress: '#f6b144',
		system: '#f06565',
		'file-history-snapshot': '#4f6ef7',
		'last-prompt': '#6b7594'
	};

	let typeCounts = $derived(
		records.reduce(
			(acc, r) => {
				acc[r.record_type] = (acc[r.record_type] || 0) + 1;
				return acc;
			},
			{} as Record<string, number>
		)
	);

	let activeFilters = $state<Set<string>>(new Set());
	let searchQuery = $state('');

	function toggleFilter(type: string) {
		const next = new Set(activeFilters);
		if (next.has(type)) next.delete(type);
		else next.add(type);
		activeFilters = next;
	}

	let filteredRecords = $derived(
		(activeFilters.size === 0
			? records
			: records.filter((r) => activeFilters.has(r.record_type))
		)
			.filter((r) => r.text && r.text.trim().length > 0)
			.filter((r) => {
				if (!searchQuery.trim()) return true;
				const q = searchQuery.toLowerCase();
				return (
					r.text?.toLowerCase().includes(q) ||
					r.tool_name?.toLowerCase().includes(q) ||
					r.record_type.toLowerCase().includes(q)
				);
			})
	);
</script>

<div>
	<div class="mb-3 flex items-center gap-2">
		<span class="text-sm font-semibold">Transcript ({records.length} records)</span>
	</div>

	<div class="mb-3">
		<input
			type="text"
			placeholder="Search transcript..."
			bind:value={searchQuery}
			class="border-border bg-background text-foreground placeholder:text-muted-foreground w-full rounded-md border px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-primary"
		/>
	</div>

	<div class="mb-3 flex flex-wrap gap-2">
		{#each Object.entries(typeCounts) as [type, count]}
			{@const color = TYPE_COLORS[type] || '#6b7594'}
			{@const isActive = activeFilters.size === 0 || activeFilters.has(type)}
			<button
				class="rounded-full border px-2.5 py-1 text-[11px] transition-opacity"
				style="background: {color}15; color: {color}; border-color: {color}30; opacity: {isActive
					? 1
					: 0.4}"
				onclick={() => toggleFilter(type)}
			>
				{type} ({count})
			</button>
		{/each}
	</div>

	<div class="space-y-2 rounded-lg border border-border p-4">
		{#each filteredRecords as record}
			<div
				class="max-w-[85%] rounded-lg px-3 py-2 text-xs
					{record.record_type === 'user'
					? 'bg-primary/10 mr-auto'
					: record.record_type === 'assistant'
						? 'bg-muted ml-auto'
						: 'bg-muted/50 mr-auto'}"
			>
				<div class="text-muted-foreground mb-1 flex items-center gap-2 text-[10px] font-medium uppercase">
					<span>{record.record_type}</span>
					{#if record.timestamp}
						<span class="font-mono normal-case">{fmtTime(record.timestamp)}</span>
					{/if}
				</div>
				<div class="whitespace-pre-wrap break-words">{record.text?.trim()}</div>
			</div>
		{/each}

		{#if filteredRecords.length === 0}
			<div class="text-muted-foreground p-4 text-center text-sm">No matching records</div>
		{/if}
	</div>
</div>
