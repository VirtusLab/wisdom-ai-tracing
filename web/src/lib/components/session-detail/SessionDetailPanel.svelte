<script lang="ts">
	import { page } from '$app/stores';
	import { api } from '$lib/api';
	import SessionSummaryStats from './SessionSummaryStats.svelte';
	import SessionCharts from './SessionCharts.svelte';
	import SessionTranscript from './SessionTranscript.svelte';

	interface Props {
		sessionId: string;
	}

	let { sessionId }: Props = $props();

	const slug = $derived($page.params.slug);

	let data: any = $state(null);
	let loading = $state(true);
	let error = $state('');
	let showCharts = $state(true);
	let showTranscript = $state(false);

	async function fetchDetail() {
		loading = true;
		error = '';
		try {
			data = await api.get(`/api/v1/orgs/${slug}/analytics/sessions/${sessionId}/detail`);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load session detail';
		} finally {
			loading = false;
		}
	}

	$effect(() => {
		fetchDetail();
	});
</script>

<div class="bg-muted/20 border-border border-t px-4 py-4">
	{#if loading}
		<div class="text-muted-foreground flex items-center justify-center gap-2 py-8 text-sm">
			<span
				class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"
			></span>
			Loading session detail...
		</div>
	{:else if error}
		<div class="text-destructive py-4 text-center text-sm">{error}</div>
	{:else if data}
		<SessionSummaryStats
			totalTokens={data.input_tokens + data.output_tokens + data.cache_read_tokens + data.cache_write_tokens || data.total_tokens}
			estimatedCostUsd={data.estimated_cost_usd}
			outputTokens={data.output_tokens}
			apiCalls={data.api_calls}
			cacheSavings={data.cache_savings}
			compactions={data.compactions}
			costBreakdown={data.cost_breakdown}
		/>

		<div class="mt-4 space-y-3">
			<div class="border-border overflow-hidden rounded-lg border">
				<button
					class="hover:bg-muted/40 flex w-full items-center gap-3 px-4 py-3 text-left transition-colors"
					onclick={() => (showCharts = !showCharts)}
				>
					<span class="text-muted-foreground/50 text-xs">{showCharts ? '▼' : '▶'}</span>
					<span class="text-sm font-semibold">Charts</span>
				</button>
				{#if showCharts}
					<div class="border-border border-t px-4 py-4">
						<SessionCharts perCall={data.per_call} tokenDistribution={data.token_distribution} />
					</div>
				{/if}
			</div>

			<div class="border-border overflow-hidden rounded-lg border">
				<button
					class="hover:bg-muted/40 flex w-full items-center gap-3 px-4 py-3 text-left transition-colors"
					onclick={() => (showTranscript = !showTranscript)}
				>
					<span class="text-muted-foreground/50 text-xs">{showTranscript ? '▼' : '▶'}</span>
					<span class="text-sm font-semibold">Transcript</span>
					<span class="text-muted-foreground ml-auto text-xs">{data.transcript_records.length} records</span>
				</button>
				{#if showTranscript}
					<div class="border-border border-t px-4 py-4">
						<SessionTranscript records={data.transcript_records} />
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>
