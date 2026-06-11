<script lang="ts">
	import { formatTime } from '$lib/utils/date';

	interface RecordUsage {
		input_tokens: number;
		output_tokens: number;
		cache_read_tokens: number;
		cache_write_tokens: number;
		cost_usd: number;
	}

	interface TranscriptRecordData {
		record_type: string;
		timestamp: string | null;
		content_types: string[];
		tool_name: string | null;
		text: string | null;
		usage: RecordUsage | null;
		model: string | null;
	}

	interface Props {
		record: TranscriptRecordData;
	}

	let { record }: Props = $props();
	let expanded = $state(false);

	const TYPE_COLORS: Record<string, string> = {
		user: '#3ecf8e',
		assistant: '#a78bfa',
		progress: '#f6b144',
		system: '#f06565',
		'file-history-snapshot': '#4f6ef7',
		'last-prompt': '#6b7594'
	};

	const CONTENT_TYPE_COLORS: Record<string, string> = {
		thinking: '#a78bfa',
		tool_use: '#f6b144',
		text: '#3ecf8e',
		tool_result: '#4f6ef7',
		error: '#f06565'
	};

	function fmtTime(ts: string | null): string {
		if (!ts) return '';
		try {
			return formatTime(ts);
		} catch {
			return '';
		}
	}

	function fmtNum(n: number): string {
		if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
		if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
		return String(n);
	}

	function usageSummary(u: RecordUsage): string {
		const parts = [];
		if (u.output_tokens > 0) parts.push(`${fmtNum(u.output_tokens)} out`);
		if (u.cache_write_tokens > 0) parts.push(`${fmtNum(u.cache_write_tokens)} cache_w`);
		if (u.cache_read_tokens > 0) parts.push(`${fmtNum(u.cache_read_tokens)} cache_r`);
		return parts.join(' · ');
	}

	function previewText(text: string | null): string {
		if (!text) return '';
		const clean = text.replace(/\n/g, ' ').trim();
		return clean.length > 120 ? clean.slice(0, 120) + '...' : clean;
	}

	const color = $derived(TYPE_COLORS[record.record_type] || '#6b7594');
	const maxTokens = $derived(
		record.usage
			? Math.max(record.usage.cache_read_tokens, record.usage.cache_write_tokens, record.usage.output_tokens, record.usage.input_tokens, 1)
			: 1
	);
</script>

<div class="border-border border-b last:border-b-0">
	<button
		class="hover:bg-muted/50 flex w-full items-center gap-2 px-3 py-2 text-left text-xs"
		onclick={() => (expanded = !expanded)}
	>
		<span style="color: {color}" class="text-[10px]">●</span>
		<span style="color: {color}" class="min-w-[60px] text-[11px] font-medium">{record.record_type}</span>
		<span class="text-muted-foreground font-mono text-[11px]">{fmtTime(record.timestamp)}</span>

		{#if record.content_types.length > 0}
			<span class="flex gap-1">
				{#each record.content_types as ct}
					<span
						class="rounded px-1.5 py-0.5 text-[10px]"
						style="background: {CONTENT_TYPE_COLORS[ct] || '#6b7594'}20; color: {CONTENT_TYPE_COLORS[ct] || '#6b7594'}"
					>
						{ct}
					</span>
				{/each}
			</span>
		{/if}

		{#if record.usage}
			<span class="text-muted-foreground ml-auto text-[11px]">{usageSummary(record.usage)}</span>
		{/if}

		{#if !record.usage && record.text}
			<span class="text-foreground flex-1 truncate">{previewText(record.text)}</span>
		{/if}

		<span class="text-muted-foreground/50 ml-auto">{expanded ? '▼' : '▶'}</span>
	</button>

	{#if expanded}
		<div class="bg-muted/30 border-border border-t px-4 py-3 pl-8">
			{#if record.text}
				<pre class="bg-background border-border max-h-[300px] overflow-auto rounded-md border p-3 font-mono text-[11px] leading-relaxed whitespace-pre-wrap">{record.text}</pre>
			{/if}

			{#if record.usage}
				<div class="mt-3 grid grid-cols-[80px_1fr_60px] items-center gap-1 text-[11px]">
					<span class="text-muted-foreground">cache_read</span>
					<div class="bg-muted h-2 overflow-hidden rounded">
						<div
							class="h-full rounded"
							style="background: rgba(62,207,142,0.6); width: {(record.usage.cache_read_tokens / maxTokens) * 100}%"
						></div>
					</div>
					<span class="text-muted-foreground text-right">{fmtNum(record.usage.cache_read_tokens)}</span>

					<span class="text-muted-foreground">cache_write</span>
					<div class="bg-muted h-2 overflow-hidden rounded">
						<div
							class="h-full rounded"
							style="background: rgba(246,177,68,0.6); width: {(record.usage.cache_write_tokens / maxTokens) * 100}%"
						></div>
					</div>
					<span class="text-muted-foreground text-right">{fmtNum(record.usage.cache_write_tokens)}</span>

					<span class="text-muted-foreground">output</span>
					<div class="bg-muted h-2 overflow-hidden rounded">
						<div
							class="h-full rounded"
							style="background: rgba(167,139,250,0.6); width: {(record.usage.output_tokens / maxTokens) * 100}%"
						></div>
					</div>
					<span class="text-muted-foreground text-right">{fmtNum(record.usage.output_tokens)}</span>

					<span class="text-muted-foreground">input</span>
					<div class="bg-muted h-2 overflow-hidden rounded">
						<div
							class="h-full rounded"
							style="background: rgba(79,110,247,0.6); width: {(record.usage.input_tokens / maxTokens) * 100}%"
						></div>
					</div>
					<span class="text-muted-foreground text-right">{fmtNum(record.usage.input_tokens)}</span>
				</div>
			{/if}
		</div>
	{/if}
</div>
