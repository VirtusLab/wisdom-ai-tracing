<script lang="ts">
	import { page } from '$app/stores';
	import { useFetch } from '$lib/hooks/use-fetch.svelte';
	import { fmtNum, fmtCost, fmtDuration, fmtRelativeTime } from '$lib/utils/format';
	import StatCard from '$lib/components/StatCard.svelte';
	import SplitStatCard from '$lib/components/SplitStatCard.svelte';
	import HelpTip from '$lib/components/HelpTip.svelte';
	import DataTable from '$lib/components/DataTable.svelte';
	import LoadingState from '$lib/components/LoadingState.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import SessionDetailPanel from '$lib/components/session-detail/SessionDetailPanel.svelte';
	import MonitorPlayIcon from '@lucide/svelte/icons/monitor-play';
	import CoinsIcon from '@lucide/svelte/icons/coins';
	import DollarSignIcon from '@lucide/svelte/icons/dollar-sign';
	import ClockIcon from '@lucide/svelte/icons/clock';
	import WrenchIcon from '@lucide/svelte/icons/wrench';

	const COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899', '#06b6d4', '#84cc16'];

	interface ModelPref {
		model: string;
		sessions: number;
	}
	interface RecentSession {
		id: string;
		session_id: string;
		repo_name: string;
		started_at: string | null;
		duration_ms: number | null;
		cost_usd: number | null;
		model: string | null;
	}
	interface AiToolEntry {
		category: string;
		name: string;
		count: number;
	}
	interface AuthorDetailResponse {
		user: { user_id: string; email: string; name: string | null };
		sessions: number;
		tokens: number;
		input_tokens: number;
		output_tokens: number;
		cache_read_tokens: number;
		cache_write_tokens: number;
		cost_usd: number;
		avg_duration_ms: number | null;
		total_tool_calls: number;
		model_preferences: ModelPref[];
		top_software: string[];
		top_ai_tools: AiToolEntry[];
		recent_sessions: RecentSession[];
	}

	// Sonnet rate weights for proportional cost split (same as main analytics page)
	const rates = { input: 3, output: 15, cacheWrite: 3.75, cacheRead: 0.3 };

	let expandedSessionId = $state<string | null>(null);

	const slug = $derived($page.params.slug);
	const userId = $derived($page.params.user_id);
	const search = $derived($page.url.search.replace(/^\?/, ''));

	const authorQuery = useFetch<AuthorDetailResponse>(
		() => `/api/v1/orgs/${slug}/analytics/authors/${userId}` + (search ? '?' + search : '')
	);

	const sessionColumns = [
		{ key: 'session_id', label: 'Session ID' },
		{ key: 'repo_name', label: 'Repo', sortable: true },
		{ key: 'duration_ms', label: 'Duration', sortable: true },
		{ key: 'cost_usd', label: 'Cost', sortable: true },
		{ key: 'model', label: 'Model' },
		{ key: 'started_at', label: 'Started', sortable: true }
	];
</script>

<svelte:head>
	<title>{authorQuery.data ? (authorQuery.data.user.name ?? authorQuery.data.user.email) : 'User'} - Author Analytics - TraceVault</title>
</svelte:head>

<div class="space-y-6">
	{#if authorQuery.loading}
		<LoadingState />
	{:else if authorQuery.error}
		<ErrorState message={authorQuery.error} onRetry={authorQuery.refetch} />
	{:else if authorQuery.data}
		{@const data = authorQuery.data}
		<div class="flex items-center gap-3">
			<a href="/orgs/{slug}/analytics/authors" class="text-muted-foreground hover:text-foreground text-sm">&larr; Back</a>
			<h1 class="text-xl font-semibold">{data.user.name ?? data.user.email}</h1>
			{#if data.user.name}
				<span class="text-muted-foreground text-sm">{data.user.email}</span>
			{/if}
		</div>

		{@const tokenRows = [
				{ label: 'Input', value: fmtNum(data.input_tokens) },
				{ label: 'Output', value: fmtNum(data.output_tokens) },
				{ label: 'Cache write', value: fmtNum(data.cache_write_tokens) },
				{ label: 'Cache read', value: fmtNum(data.cache_read_tokens) }
			]}
		{@const weighted =
				data.input_tokens * rates.input +
				data.output_tokens * rates.output +
				data.cache_write_tokens * rates.cacheWrite +
				data.cache_read_tokens * rates.cacheRead}
		{@const c = data.cost_usd}
		{@const costRows = weighted > 0 ? [
				{ label: 'Input', value: fmtCost((data.input_tokens * rates.input / weighted) * c) },
				{ label: 'Output', value: fmtCost((data.output_tokens * rates.output / weighted) * c) },
				{ label: 'Cache write', value: fmtCost((data.cache_write_tokens * rates.cacheWrite / weighted) * c) },
				{ label: 'Cache read', value: fmtCost((data.cache_read_tokens * rates.cacheRead / weighted) * c) }
			] : []}
		<div class="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-5">
			<StatCard label="Sessions" value={fmtNum(data.sessions)} icon={MonitorPlayIcon} color="#3b82f6" tooltip="Total AI coding sessions." />
			<SplitStatCard label="Tokens" value={fmtNum(data.tokens)} icon={CoinsIcon} color="#f59e0b" tooltip="Total tokens consumed across all sessions." rows={tokenRows} />
			<SplitStatCard label="Cost" value={fmtCost(data.cost_usd)} icon={DollarSignIcon} color="#dc2626" tooltip="Total estimated cost based on token usage and model pricing." rows={costRows} />
			<StatCard label="Avg Duration" value={fmtDuration(data.avg_duration_ms)} icon={ClockIcon} color="#06b6d4" tooltip="Average session duration." />
			<StatCard label="Tool Calls" value={fmtNum(data.total_tool_calls)} icon={WrenchIcon} color="#10b981" tooltip="Total tool invocations." />
		</div>

		{#if data.model_preferences.length > 0}
			<div class="border-border rounded-lg border p-3">
				<h4 class="mb-2 text-sm font-semibold">Model Preferences</h4>
				<div class="flex flex-wrap gap-2">
					{#each data.model_preferences as pref}
						<span class="rounded-full px-3 py-1 text-xs font-medium" style="background: rgba(167,139,250,0.12); color: #a78bfa; border: 1px solid rgba(167,139,250,0.25)">{pref.model} ({pref.sessions})</span>
					{/each}
				</div>
			</div>
		{/if}

		{#if data.top_software.length > 0 || (data.top_ai_tools ?? []).length > 0}
			<div class="border-border rounded-lg border p-3 space-y-3">
				{#if data.top_software.length > 0}
					<div>
						<h4 class="text-sm font-semibold mb-2">Top Software <HelpTip text="Most frequently used CLI tools." /></h4>
						<div class="flex flex-wrap gap-2">
							{#each data.top_software as tool, i}
								<span class="rounded-full px-3 py-1 text-xs font-medium" style="background: {COLORS[i % COLORS.length]}22; color: {COLORS[i % COLORS.length]}; border: 1px solid {COLORS[i % COLORS.length]}40">{tool}</span>
							{/each}
						</div>
					</div>
				{/if}
				{#if (data.top_ai_tools ?? []).length > 0}
					<div>
						<h4 class="text-sm font-semibold mb-2">Top AI Tools <HelpTip text="Most frequently used MCP servers and skill groups." /></h4>
						<div class="flex flex-wrap gap-2">
							{#each data.top_ai_tools as tool, i}
								<span class="rounded-full px-3 py-1 text-xs font-medium" style="background: {COLORS[i % COLORS.length]}22; color: {COLORS[i % COLORS.length]}; border: 1px solid {COLORS[i % COLORS.length]}40">
									<span class="text-muted-foreground">{tool.category === 'mcp_server' ? 'MCP' : 'Skill'}:</span> {tool.name} ({tool.count})
								</span>
							{/each}
						</div>
					</div>
				{/if}
				<div class="flex justify-end">
					<a href="/orgs/{slug}/analytics/software/users/{userId}" class="text-xs text-primary hover:underline">View all software & AI tools &rarr;</a>
				</div>
			</div>
		{/if}

		<div>
			<h4 class="mb-2 text-sm font-semibold">Recent Sessions <HelpTip text="Latest coding sessions." /></h4>
			<DataTable
				columns={sessionColumns}
				rows={data.recent_sessions}
				searchKeys={['session_id', 'repo_name']}
				defaultSort="started_at"
				defaultSortDir="desc"
				rowIdKey="id"
				onRowClick={(row) => {
					const id = row.id as string;
					expandedSessionId = expandedSessionId === id ? null : id;
				}}
				expandedRowId={expandedSessionId}
			>
				{#snippet children({ row, col })}
					{#if col.key === 'session_id'}
						<span class="font-mono">{(row.session_id as string).slice(0, 8)}</span>
					{:else if col.key === 'duration_ms'}
						<span class="font-mono">{fmtDuration(row.duration_ms as number | null)}</span>
					{:else if col.key === 'cost_usd'}
						<span class="font-mono">{fmtCost(row.cost_usd as number | null)}</span>
					{:else if col.key === 'model'}
						{#if row.model}
							<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(79,110,247,0.12); color: #4f6ef7; border: 1px solid rgba(79,110,247,0.25)">{row.model}</span>
						{:else}
							<span class="text-muted-foreground">-</span>
						{/if}
					{:else if col.key === 'started_at'}
						{fmtRelativeTime(row.started_at as string | null)}
					{:else}
						{row[col.key] ?? '-'}
					{/if}
				{/snippet}
				{#snippet expandedRow({ row })}
					<SessionDetailPanel sessionId={row.id as string} />
				{/snippet}
			</DataTable>
		</div>
	{/if}
</div>
