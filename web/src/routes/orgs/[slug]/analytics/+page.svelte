<script lang="ts">
	import { page } from '$app/stores';
	import { useFetch } from '$lib/hooks/use-fetch.svelte';
	import { fmtNum, fmtCost, fmtDuration } from '$lib/utils/format';
	import StatCard from '$lib/components/StatCard.svelte';
	import SplitStatCard from '$lib/components/SplitStatCard.svelte';
	import HelpTip from '$lib/components/HelpTip.svelte';
	import DataTable from '$lib/components/DataTable.svelte';
	import Chart from '$lib/components/chart.svelte';
	import LoadingState from '$lib/components/LoadingState.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import GitCommitHorizontalIcon from '@lucide/svelte/icons/git-commit-horizontal';
	import MonitorPlayIcon from '@lucide/svelte/icons/monitor-play';
	import CoinsIcon from '@lucide/svelte/icons/coins';
	import UsersIcon from '@lucide/svelte/icons/users';
	import PercentIcon from '@lucide/svelte/icons/percent';
	import DollarSignIcon from '@lucide/svelte/icons/dollar-sign';
	import { formatDate } from '$lib/utils/date';
	import ClockIcon from '@lucide/svelte/icons/clock';
	import WrenchIcon from '@lucide/svelte/icons/wrench';
	import PiggyBankIcon from '@lucide/svelte/icons/piggy-bank';
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

	const COLORS = ['#dc2626', '#3b82f6', '#10b981', '#f59e0b', '#8b5cf6', '#ec4899', '#06b6d4', '#84cc16'];

	interface TimeSeriesPoint {
		date: string;
		input: number;
		output: number;
	}
	interface RepoTokens {
		repo: string;
		tokens: number;
	}
	interface ModelCount {
		model: string;
		count: number;
	}
	interface RecentCommit {
		commit_sha: string;
		author: string;
		session_count: number;
		total_tokens: number;
		created_at: string;
	}
	interface SessionTimePoint {
		date: string;
		count: number;
	}
	interface HourlyActivity {
		hour: number;
		count: number;
	}
	interface OverviewResponse {
		total_commits: number;
		total_sessions: number;
		total_tokens: number;
		total_input_tokens: number;
		total_output_tokens: number;
		active_authors: number;
		estimated_cost_usd: number;
		ai_percentage: number | null;
		total_duration_ms: number;
		avg_session_duration_ms: number | null;
		total_tool_calls: number;
		total_compactions: number;
		total_compaction_tokens_saved: number;
		total_cache_read_tokens: number;
		total_cache_write_tokens: number;
		cache_savings_usd: number;
		tokens_over_time: TimeSeriesPoint[];
		sessions_over_time: SessionTimePoint[];
		hourly_activity: HourlyActivity[];
		top_repos: RepoTokens[];
		model_distribution: ModelCount[];
		recent_commits: RecentCommit[];
	}

	const slug = $derived($page.params.slug);
	const search = $derived($page.url.search.replace(/^\?/, ''));

	const overview = useFetch<OverviewResponse>(
		() => `/api/v1/orgs/${slug}/analytics/overview` + (search ? '?' + search : '')
	);

	function tokensChartData(d: OverviewResponse) {
		return {
			labels: d.tokens_over_time.map((p) => p.date),
			datasets: [
				{
					label: 'Input Tokens',
					data: d.tokens_over_time.map((p) => p.input),
					borderColor: '#3b82f6',
					backgroundColor: 'rgba(59,130,246,0.1)',
					fill: true,
					tension: 0.3
				},
				{
					label: 'Output Tokens',
					data: d.tokens_over_time.map((p) => p.output),
					borderColor: '#10b981',
					backgroundColor: 'rgba(16,185,129,0.1)',
					fill: true,
					tension: 0.3
				}
			]
		};
	}

	function reposChartData(d: OverviewResponse) {
		return {
			labels: d.top_repos.map((r) => r.repo),
			datasets: [
				{
					label: 'Tokens',
					data: d.top_repos.map((r) => r.tokens),
					backgroundColor: COLORS.slice(0, d.top_repos.length)
				}
			]
		};
	}

	function modelChartData(d: OverviewResponse) {
		return {
			labels: d.model_distribution.map((m) => m.model),
			datasets: [
				{
					data: d.model_distribution.map((m) => m.count),
					backgroundColor: COLORS.slice(0, d.model_distribution.length)
				}
			]
		};
	}

	function hourlyActivityChartData(d: OverviewResponse) {
		const hours = Array.from({ length: 24 }, (_, i) => i);
		const counts = hours.map((h) => d.hourly_activity.find((a) => a.hour === h)?.count ?? 0);
		return {
			labels: hours.map((h) => String(h)),
			datasets: [
				{
					label: 'Sessions',
					data: counts,
					backgroundColor: '#8b5cf6'
				}
			]
		};
	}

	function sessionsOverTimeChartData(d: OverviewResponse) {
		return {
			labels: d.sessions_over_time.map((p) => p.date),
			datasets: [
				{
					label: 'Sessions',
					data: d.sessions_over_time.map((p) => p.count),
					borderColor: '#f59e0b',
					backgroundColor: 'rgba(245,158,11,0.1)',
					fill: true,
					tension: 0.3
				}
			]
		};
	}

	const tokensSecondary = $derived.by(() => {
		const d = overview.data;
		if (!d || d.total_sessions === 0) return undefined;
		return `${fmtNum(Math.round(d.total_tokens / d.total_sessions))} avg/session`;
	});

	const costSecondary = $derived.by(() => {
		const d = overview.data;
		if (!d || d.total_commits === 0) return undefined;
		return `$${(d.estimated_cost_usd / d.total_commits).toFixed(2)} avg/commit`;
	});

	const tokenRows = $derived.by(() => {
		const d = overview.data;
		if (!d) return [];
		return [
			{ label: 'Input', value: fmtNum(d.total_input_tokens) },
			{ label: 'Output', value: fmtNum(d.total_output_tokens) },
			{ label: 'Cache write', value: fmtNum(d.total_cache_write_tokens) },
			{ label: 'Cache read', value: fmtNum(d.total_cache_read_tokens) }
		];
	});

	const costRows = $derived.by(() => {
		const d = overview.data;
		if (!d) return [];
		// Weight by Sonnet rates to apportion total cost per type (approximate — model mix varies)
		const rates = { input: 3.0, output: 15.0, cacheWrite: 3.75, cacheRead: 0.30 };
		const weighted =
			d.total_input_tokens * rates.input +
			d.total_output_tokens * rates.output +
			d.total_cache_write_tokens * rates.cacheWrite +
			d.total_cache_read_tokens * rates.cacheRead;
		if (weighted === 0) return [];
		const c = d.estimated_cost_usd;
		return [
			{ label: 'Input', value: fmtCost(((d.total_input_tokens * rates.input) / weighted) * c) },
			{ label: 'Output', value: fmtCost(((d.total_output_tokens * rates.output) / weighted) * c) },
			{ label: 'Cache write', value: fmtCost(((d.total_cache_write_tokens * rates.cacheWrite) / weighted) * c) },
			{ label: 'Cache read', value: fmtCost(((d.total_cache_read_tokens * rates.cacheRead) / weighted) * c) }
		];
	});

	const savingsRows = $derived.by(() => {
		const d = overview.data;
		if (!d || d.cache_savings_usd === 0) return [];
		const fullCost = d.estimated_cost_usd + d.cache_savings_usd;
		const pct = (d.cache_savings_usd / fullCost) * 100;
		return [
			{ label: 'Full cost', value: fmtCost(fullCost) },
			{ label: 'Saved', value: `${pct.toFixed(1)}%` }
		];
	});

	const commitColumns = [
		{ key: 'commit_sha', label: 'Commit' },
		{ key: 'author', label: 'Author' },
		{ key: 'session_count', label: 'Sessions', sortable: true },
		{ key: 'total_tokens', label: 'Tokens', sortable: true },
		{ key: 'created_at', label: 'Date', sortable: true }
	];
</script>

<svelte:head>
	<title>Analytics - TraceVault</title>
</svelte:head>

<div class="space-y-6">
	<h1 class="text-xl font-semibold">Analytics Overview</h1>

	{#if overview.loading}
		<LoadingState />
	{:else if overview.error}
		<ErrorState message={overview.error} onRetry={overview.refetch} />
	{:else if overview.data}
		{@const data = overview.data}
		<div class="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-3 xl:grid-cols-3">
			<StatCard label="Total Commits" value={fmtNum(data.total_commits)} icon={GitCommitHorizontalIcon} color="#3b82f6" tooltip="Total git commits linked to AI sessions in the selected period." />
			<StatCard label="Sessions" value={fmtNum(data.total_sessions)} icon={MonitorPlayIcon} color="#10b981" tooltip="Total AI coding sessions in the selected period." />
			<SplitStatCard label="Total Tokens" value={fmtNum(data.total_tokens)} icon={CoinsIcon} color="#f59e0b" secondary={tokensSecondary} tooltip="Total tokens processed, including input, output, and cached tokens." rows={tokenRows} />
			<StatCard label="Active Authors" value={String(data.active_authors)} icon={UsersIcon} color="#8b5cf6" tooltip="Number of unique developers with AI sessions." />
			<StatCard label="AI %" value={data.ai_percentage != null ? `${data.ai_percentage.toFixed(1)}%` : 'N/A'} icon={PercentIcon} color="#ec4899" tooltip="Percentage of code lines attributed to AI across all commits." />
			<SplitStatCard label="Estimated Cost" value={fmtCost(data.estimated_cost_usd)} icon={DollarSignIcon} color="#dc2626" secondary={costSecondary} tooltip="Estimated total cost based on token usage and model pricing." rows={costRows} />
			<StatCard label="Avg Session Duration" value={fmtDuration(data.avg_session_duration_ms)} icon={ClockIcon} color="#06b6d4" tooltip="Average wall-clock time of completed sessions." />
			<StatCard label="Total Tool Calls" value={fmtNum(data.total_tool_calls)} icon={WrenchIcon} color="#f59e0b" tooltip="Total tool invocations across all sessions (edits, reads, bash, etc.)." />
			<SplitStatCard label="Cache Savings" value={fmtCost(data.cache_savings_usd)} icon={PiggyBankIcon} color="#10b981" tooltip="Money saved by reusing cached tokens at reduced rates. Full cost shows what you would have paid without caching." rows={savingsRows} />
		</div>

		<div class="grid gap-4 lg:grid-cols-2">
			<div class="border-border rounded-lg border p-3">
				<h4 class="mb-2 text-sm font-semibold">
					<a href="/orgs/{slug}/analytics/tokens" class="hover:underline">Tokens Over Time</a>
					<HelpTip text="Input and output token usage per day over the selected period." />
				</h4>
				{#if data.tokens_over_time.length > 0}
					<div style="height: 200px">
						<Chart
							type="line"
							data={tokensChartData(data)}
							options={{ responsive: true, maintainAspectRatio: false, plugins: { legend: { position: 'bottom', labels: { boxWidth: 12, font: { size: 11 } } } } }}
						/>
					</div>
				{:else}
					<p class="text-muted-foreground text-sm">No data</p>
				{/if}
			</div>

			<div class="border-border rounded-lg border p-3">
				<h4 class="mb-2 text-sm font-semibold">
					<a href="/orgs/{slug}/analytics/tokens" class="hover:underline">Top Repos by Tokens</a>
					<HelpTip text="Repositories ranked by total token consumption." />
				</h4>
				{#if data.top_repos.length > 0}
					<div style="height: 200px">
						<Chart
							type="bar"
							data={reposChartData(data)}
							options={{ responsive: true, maintainAspectRatio: false, indexAxis: 'y', plugins: { legend: { display: false } } }}
						/>
					</div>
				{:else}
					<p class="text-muted-foreground text-sm">No data</p>
				{/if}
			</div>
		</div>

		<div class="grid gap-4 lg:grid-cols-2">
			<div class="border-border rounded-lg border p-3">
				<h4 class="mb-2 text-sm font-semibold">
					<a href="/orgs/{slug}/analytics/sessions" class="hover:underline">Hourly Activity</a>
					<HelpTip text="Distribution of sessions by hour of day (UTC), showing peak coding times." />
				</h4>
				{#if data.hourly_activity.length > 0}
					<div style="height: 200px">
						<Chart
							type="bar"
							data={hourlyActivityChartData(data)}
							options={{
								responsive: true,
								maintainAspectRatio: false,
								plugins: { legend: { display: false } },
								scales: { x: { title: { display: true, text: 'Hour of Day' } }, y: { title: { display: true, text: 'Sessions' } } }
							}}
						/>
					</div>
				{:else}
					<p class="text-muted-foreground text-sm">No data</p>
				{/if}
			</div>

			<div class="border-border rounded-lg border p-3">
				<h4 class="mb-2 text-sm font-semibold">
					<a href="/orgs/{slug}/analytics/sessions" class="hover:underline">Sessions Over Time</a>
					<HelpTip text="Number of sessions started per day." />
				</h4>
				{#if data.sessions_over_time.length > 0}
					<div style="height: 200px">
						<Chart
							type="line"
							data={sessionsOverTimeChartData(data)}
							options={{ responsive: true, maintainAspectRatio: false, plugins: { legend: { position: 'bottom', labels: { boxWidth: 12, font: { size: 11 } } } } }}
						/>
					</div>
				{:else}
					<p class="text-muted-foreground text-sm">No data</p>
				{/if}
			</div>
		</div>

		<div class="grid gap-4 lg:grid-cols-2">
			<div class="border-border rounded-lg border p-3">
				<h4 class="mb-2 text-sm font-semibold">
					<a href="/orgs/{slug}/analytics/models" class="hover:underline">Model Distribution</a>
					<HelpTip text="Breakdown of sessions by AI model used." />
				</h4>
				{#if data.model_distribution.length > 0}
					<div style="height: 200px">
						<Chart
							type="doughnut"
							data={modelChartData(data)}
							options={{ responsive: true, maintainAspectRatio: false, cutout: '65%', plugins: { legend: { position: 'bottom', labels: { boxWidth: 12, font: { size: 11 } } } } } as any}
						/>
					</div>
				{:else}
					<p class="text-muted-foreground text-sm">No data</p>
				{/if}
			</div>

			<div class="border-border rounded-lg border p-3">
				<h4 class="mb-2 text-sm font-semibold">Recent Commits</h4>
				<DataTable
					columns={commitColumns}
					rows={data.recent_commits}
					searchKeys={['commit_sha', 'author']}
					defaultSort="created_at"
					rowIdKey="commit_sha"
				>
					{#snippet children({ row, col })}
						{#if col.key === 'commit_sha'}
							<span class="font-mono">{String(row.commit_sha).slice(0, 8)}</span>
						{:else if col.key === 'total_tokens'}
							<span class="font-mono">{fmtNum(row.total_tokens as number)}</span>
						{:else if col.key === 'created_at'}
							{formatDate(row.created_at as string)}
						{:else}
							{row[col.key] ?? '-'}
						{/if}
					{/snippet}
				</DataTable>
			</div>
		</div>
	{/if}
</div>
