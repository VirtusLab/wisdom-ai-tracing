<script lang="ts">
	import * as Table from '$lib/components/ui/table/index.js';
	import * as Select from '$lib/components/ui/select/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { formatDate } from '$lib/utils/date';
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import { api } from '$lib/api';

	interface Policy {
		id: string;
		name: string;
		condition: Record<string, unknown>;
	}

	interface PolicyEvaluation {
		id: string;
		policy_id: string | null;
		policy_name: string;
		session_id: string | null;
		commit_sha: string | null;
		result: string;
		action: string;
		details: string;
		source: string;
		actor_id: string | null;
		evaluated_at: string;
		is_synthetic: boolean;
	}

	interface PolicyEvaluationPage {
		items: PolicyEvaluation[];
		total: number;
	}

	interface Props {
		slug: string;
		repoId: string;
		policies: Policy[];
	}

	let { slug, repoId, policies }: Props = $props();

	let evaluations: PolicyEvaluation[] = $state([]);
	let evaluationsTotal = $state(0);
	let loading = $state(true);
	let error = $state('');
	let filterResult = $state('all');
	let filterPolicyId = $state('all');
	let pageSize = $state(25);
	let page = $state(0);

	async function load() {
		loading = true;
		error = '';
		try {
			const params = new URLSearchParams({
				limit: String(pageSize),
				offset: String(page * pageSize)
			});
			if (filterResult !== 'all') params.set('result', filterResult);
			if (filterPolicyId !== 'all') params.set('policy_id', filterPolicyId);
			const result = await api.get<PolicyEvaluationPage>(
				`/api/v1/orgs/${slug}/repos/${repoId}/policy-evaluations?${params}`
			);
			evaluations = result?.items ?? [];
			evaluationsTotal = result?.total ?? 0;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load policy activity';
		} finally {
			loading = false;
		}
	}

	function setPage(p: number) {
		page = p;
		load();
	}

	function resultPillStyle(result: string): string {
		switch (result) {
			case 'pass':   return 'background: rgba(34,197,94,0.12); color: #22c55e; border: 1px solid rgba(34,197,94,0.25)';
			case 'fail':   return 'background: rgba(240,101,101,0.12); color: #f06565; border: 1px solid rgba(240,101,101,0.25)';
			case 'warn':   return 'background: rgba(246,177,68,0.12); color: #f6b144; border: 1px solid rgba(246,177,68,0.25)';
			case 'skip':   return 'background: rgba(148,163,184,0.12); color: #94a3b8; border: 1px solid rgba(148,163,184,0.25)';
			default:       return 'background: rgba(148,163,184,0.12); color: #94a3b8';
		}
	}

	// Load on mount
	$effect(() => { load(); });
</script>

<div class="border-border overflow-hidden rounded-lg border">
	<div class="bg-muted/30 flex items-center justify-between gap-3 px-4 py-3">
		<span class="text-sm font-semibold">Policy Activity</span>
		<div class="flex items-center gap-2">
			<Select.Root type="single" value={filterResult} onValueChange={(v) => { if (v) { filterResult = v; page = 0; load(); } }}>
				<Select.Trigger class="h-8 w-[120px] text-xs">
					{filterResult === 'all' ? 'All results' : filterResult}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="all">All results</Select.Item>
					<Select.Item value="pass">pass</Select.Item>
					<Select.Item value="fail">fail</Select.Item>
					<Select.Item value="warn">warn</Select.Item>
					<Select.Item value="skip">skip</Select.Item>
				</Select.Content>
			</Select.Root>
			<Select.Root type="single" value={filterPolicyId} onValueChange={(v) => { if (v) { filterPolicyId = v; page = 0; load(); } }}>
				<Select.Trigger class="h-8 w-[180px] text-xs">
					{filterPolicyId === 'all' ? 'All rules' : (policies.find((p) => p.id === filterPolicyId)?.name ?? filterPolicyId.slice(0, 8))}
				</Select.Trigger>
				<Select.Content>
					<Select.Item value="all">All rules</Select.Item>
					{#each policies as p}
						<Select.Item value={p.id}>{p.name}</Select.Item>
					{/each}
				</Select.Content>
			</Select.Root>
			<Button size="sm" variant="outline" onclick={load}>Refresh</Button>
		</div>
	</div>
	<div class="p-4">
		{#if loading}
			<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm">
				<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"></span>
				Loading...
			</div>
		{:else if error}
			<p class="text-destructive">{error}</p>
		{:else if evaluations.length === 0}
			<p class="text-muted-foreground text-sm">No policy evaluations recorded yet. Run <code class="font-mono text-xs">tracevault check</code> from a repo with policies enabled to populate this view.</p>
		{:else}
			<div class="border-border overflow-hidden rounded-lg border">
				<Table.Root class="text-xs">
					<Table.Header>
						<Table.Row class="bg-muted/30 border-border border-b">
							<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">When</Table.Head>
							<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Rule</Table.Head>
							<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Result</Table.Head>
							<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Requires</Table.Head>
							<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Action</Table.Head>
							<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Commit</Table.Head>
							<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Session</Table.Head>
							<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Source</Table.Head>
							<Table.Head class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">Details</Table.Head>
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each evaluations as ev}
							<Table.Row class="hover:bg-muted/40 transition-colors">
								<Table.Cell class="text-xs whitespace-nowrap">{formatDate(ev.evaluated_at)}</Table.Cell>
								<Table.Cell class="text-xs font-medium">
									{ev.policy_name}
									{#if ev.policy_id === null && !ev.is_synthetic}
										<span class="text-muted-foreground text-[10px]"> (deleted)</span>
									{/if}
								</Table.Cell>
								<Table.Cell class="text-xs">
									<span class="rounded-full px-2 py-0.5 text-[10px]" style={resultPillStyle(ev.result)}>
										{ev.result}
									</span>
								</Table.Cell>
								<Table.Cell class="text-xs">
									{@const pol = policies.find((p) => p.id === ev.policy_id)}
									{@const mustSucceed = pol ? (pol.condition as Record<string, unknown>).must_succeed === true : false}
									{#if mustSucceed}
										<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(79,110,247,0.12); color: #4f6ef7; border: 1px solid rgba(79,110,247,0.25)">pass</span>
									{:else}
										<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(148,163,184,0.12); color: #94a3b8; border: 1px solid rgba(148,163,184,0.25)">call</span>
									{/if}
								</Table.Cell>
								<Table.Cell class="text-xs">
									{#if ev.action === 'block_push'}
										<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(240,101,101,0.08); color: #f06565">block</span>
									{:else}
										<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(246,177,68,0.08); color: #f6b144">warn</span>
									{/if}
								</Table.Cell>
								<Table.Cell class="font-mono text-xs">
									{ev.commit_sha ? ev.commit_sha.slice(0, 8) : '-'}
								</Table.Cell>
								<Table.Cell class="font-mono text-xs max-w-[120px] truncate" title={ev.session_id ?? ''}>
									{ev.session_id ? ev.session_id.slice(0, 8) : '-'}
								</Table.Cell>
								<Table.Cell class="text-xs text-muted-foreground">{ev.source}</Table.Cell>
								<Table.Cell class="text-xs max-w-md truncate text-muted-foreground" title={ev.details}>
									{ev.details}
								</Table.Cell>
							</Table.Row>
						{/each}
					</Table.Body>
				</Table.Root>
				{#if evaluationsTotal > 0}
					{@const totalPages = Math.max(1, Math.ceil(evaluationsTotal / pageSize))}
					{@const showFrom = page * pageSize + 1}
					{@const showTo = Math.min((page + 1) * pageSize, evaluationsTotal)}
					<div class="border-border text-muted-foreground flex items-center justify-between border-t px-3 py-2 text-xs">
						<span>{showFrom}-{showTo} of {evaluationsTotal}</span>
						<div class="flex items-center gap-3">
							<span>Per page:</span>
							{#each [10, 25, 50] as size}
								<button
									class="rounded px-1.5 py-0.5 transition-colors {pageSize === size ? 'bg-primary text-primary-foreground' : 'hover:text-foreground'}"
									onclick={() => { pageSize = size; page = 0; load(); }}
								>
									{size}
								</button>
							{/each}
							<span class="text-border mx-1">|</span>
							<button class="hover:text-foreground disabled:opacity-30" disabled={page === 0} onclick={() => setPage(page - 1)}>
								<ChevronLeftIcon class="h-4 w-4" />
							</button>
							<span>{page + 1}/{totalPages}</span>
							<button class="hover:text-foreground disabled:opacity-30" disabled={page >= totalPages - 1} onclick={() => setPage(page + 1)}>
								<ChevronRightIcon class="h-4 w-4" />
							</button>
						</div>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>
