<script lang="ts">
	import { page } from '$app/stores';
	import { useFetch } from '$lib/hooks/use-fetch.svelte';
	import { fmtCost } from '$lib/utils/format';
	import { features } from '$lib/stores/features';
	import EnterpriseUpgrade from '$lib/components/enterprise-upgrade.svelte';
	import Chart from '$lib/components/chart.svelte';
	import StatCard from '$lib/components/StatCard.svelte';
	import HelpTip from '$lib/components/HelpTip.svelte';
	import LoadingState from '$lib/components/LoadingState.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import DollarSign from '@lucide/svelte/icons/dollar-sign';
	import PiggyBank from '@lucide/svelte/icons/piggy-bank';
	import Calculator from '@lucide/svelte/icons/calculator';
	import {
		Chart as ChartJS,
		CategoryScale,
		LinearScale,
		PointElement,
		LineElement,
		BarElement,
		ArcElement,
		Title,
		Tooltip,
		Legend,
		Filler
	} from 'chart.js';

	ChartJS.register(
		CategoryScale,
		LinearScale,
		PointElement,
		LineElement,
		BarElement,
		ArcElement,
		Title,
		Tooltip,
		Legend,
		Filler
	);

	const COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899', '#06b6d4', '#84cc16'];

	interface CostTimePoint {
		date: string;
		cost: number;
	}
	interface ModelCost {
		model: string;
		cost: number;
		tokens: number;
		sessions: number;
	}
	interface RepoCost {
		repo: string;
		cost: number;
	}
	interface AuthorCost {
		author: string;
		cost: number;
	}
	interface CostResponse {
		total_cost: number;
		avg_cost_per_session: number;
		cache_savings_usd: number;
		cost_over_time: CostTimePoint[];
		cost_by_model: ModelCost[];
		cost_by_repo: RepoCost[];
		cost_by_author: AuthorCost[];
	}

	const slug = $derived($page.params.slug);
	const search = $derived($page.url.search.replace(/^\?/, ''));

	const costQuery = useFetch<CostResponse>(
		() => `/api/v1/orgs/${slug}/analytics/cost` + (search ? '?' + search : '')
	);

	function costOverTimeChartData(d: CostResponse) {
		return {
			labels: d.cost_over_time.map((p) => p.date),
			datasets: [
				{
					label: 'Cost ($)',
					data: d.cost_over_time.map((p) => p.cost),
					borderColor: '#3b82f6',
					backgroundColor: 'rgba(59,130,246,0.1)',
					fill: true,
					tension: 0.3
				}
			]
		};
	}

	function costByModelChartData(d: CostResponse) {
		return {
			labels: d.cost_by_model.map((m) => m.model),
			datasets: [
				{
					data: d.cost_by_model.map((m) => m.cost),
					backgroundColor: COLORS.slice(0, d.cost_by_model.length)
				}
			]
		};
	}

	function costByRepoChartData(d: CostResponse) {
		return {
			labels: d.cost_by_repo.map((r) => r.repo),
			datasets: [
				{
					label: 'Cost ($)',
					data: d.cost_by_repo.map((r) => r.cost),
					backgroundColor: COLORS.slice(0, d.cost_by_repo.length)
				}
			]
		};
	}

	function costByAuthorChartData(d: CostResponse) {
		return {
			labels: d.cost_by_author.map((a) => a.author),
			datasets: [
				{
					label: 'Cost ($)',
					data: d.cost_by_author.map((a) => a.cost),
					backgroundColor: COLORS.slice(0, d.cost_by_author.length)
				}
			]
		};
	}
</script>

<svelte:head>
	<title>Cost Analytics - TraceVault</title>
</svelte:head>

{#if !$features.loaded}
	<LoadingState />
{:else if $features.advanced_analytics}
<div class="space-y-6">
	<h1 class="text-2xl font-bold">Cost Analytics</h1>

	{#if costQuery.loading}
		<LoadingState />
	{:else if costQuery.error}
		<ErrorState message={costQuery.error} onRetry={costQuery.refetch} />
	{:else if costQuery.data}
		{@const data = costQuery.data}
		<div class="grid grid-cols-1 gap-4 md:grid-cols-3">
			<StatCard label="Total Cost" value={fmtCost(data.total_cost)} icon={DollarSign} color="#dc2626" tooltip="Total estimated cost based on token usage and model pricing rates." />
			<StatCard label="Cache Savings" value={fmtCost(data.cache_savings_usd)} icon={PiggyBank} color="#10b981" tooltip="Net savings from prompt caching — tokens served from cache at reduced rates." />
			<StatCard label="Avg Cost/Session" value={fmtCost(data.avg_cost_per_session)} icon={Calculator} color="#3b82f6" tooltip="Average estimated cost per session." />
		</div>
		<p class="text-muted-foreground mt-2 text-[11px]">Totals include proxy API usage where available. Per-repo and per-session breakdowns reflect hook-tracked sessions only.</p>

		<div class="border-border rounded-lg border p-3">
			<h4 class="mb-2 text-sm font-semibold">Cost Over Time<HelpTip text="Daily cost trend over the selected period." /></h4>
			{#if data.cost_over_time.length > 0}
				<div style="height: 200px">
					<Chart
						type="line"
						data={costOverTimeChartData(data)}
						options={{ responsive: true, maintainAspectRatio: false, plugins: { legend: { position: 'bottom', labels: { boxWidth: 12, font: { size: 11 } } } } }}
					/>
				</div>
			{:else}
				<p class="text-muted-foreground text-sm">No data</p>
			{/if}
		</div>

		<div class="grid gap-4 lg:grid-cols-2">
			<div class="border-border rounded-lg border p-3">
				<h4 class="mb-2 text-sm font-semibold">Cost by Model<HelpTip text="Cost distribution across different AI models." /></h4>
				{#if data.cost_by_model.length > 0}
					<div style="height: 200px">
						<Chart
							type="doughnut"
							data={costByModelChartData(data)}
							options={{ responsive: true, maintainAspectRatio: false, cutout: '65%', plugins: { legend: { position: 'bottom', labels: { boxWidth: 12, font: { size: 11 } } } } } as any}
						/>
					</div>
				{:else}
					<p class="text-muted-foreground text-sm">No data</p>
				{/if}
			</div>

			<div class="border-border rounded-lg border p-3">
				<h4 class="mb-2 text-sm font-semibold">Cost by Repository<HelpTip text="Cost breakdown by repository." /></h4>
				{#if data.cost_by_repo.length > 0}
					<div style="height: 200px">
						<Chart
							type="bar"
							data={costByRepoChartData(data)}
							options={{ responsive: true, maintainAspectRatio: false, indexAxis: 'y', plugins: { legend: { display: false } } }}
						/>
					</div>
				{:else}
					<p class="text-muted-foreground text-sm">No data</p>
				{/if}
			</div>
		</div>

		<div class="border-border rounded-lg border p-3">
			<h4 class="mb-2 text-sm font-semibold">Cost by Author<HelpTip text="Cost breakdown by developer." /></h4>
			{#if data.cost_by_author.length > 0}
				<div style="height: 200px">
					<Chart
						type="bar"
						data={costByAuthorChartData(data)}
						options={{ responsive: true, maintainAspectRatio: false, indexAxis: 'y', plugins: { legend: { display: false } } }}
					/>
				</div>
			{:else}
				<p class="text-muted-foreground text-sm">No data</p>
			{/if}
		</div>
	{/if}
</div>
{:else}
	<div class="border-border overflow-hidden rounded-lg border">
		<EnterpriseUpgrade feature="advanced_analytics" />
	</div>
{/if}
