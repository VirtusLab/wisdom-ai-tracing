<script lang="ts">
	import * as Table from '$lib/components/ui/table/index.js';
	import * as Popover from '$lib/components/ui/popover/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { formatDate } from '$lib/utils/date';
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import CheckIcon from '@lucide/svelte/icons/check';
	import ShieldIcon from '@lucide/svelte/icons/shield';
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

	const ALL_RESULTS = ['pass', 'fail', 'skip'];
	const ALL_ACTIONS = ['warn', 'block_push'];
	const DATE_PRESETS = [
		{ label: 'Last 7 days', days: 7 },
		{ label: 'Last 30 days', days: 30 },
		{ label: 'Last 90 days', days: 90 },
		{ label: 'All time', days: null }
	];

	let evaluations: PolicyEvaluation[] = $state([]);
	let evaluationsTotal = $state(0);
	let loading = $state(true);
	let error = $state('');

	// Filters — default to fail results + warn action (most actionable view)
	let selectedResults: string[] = $state(['fail']);
	let selectedActions: string[] = $state(['warn']);
	let selectedPolicyIds: string[] = $state([]);  // empty = all
	let selectedDays: number | null = $state(30);

	let pageSize = $state(25);
	let page = $state(0);

	function sinceDate(): string | null {
		if (selectedDays === null) return null;
		const d = new Date();
		d.setDate(d.getDate() - selectedDays);
		return d.toISOString();
	}

	async function load() {
		loading = true;
		error = '';
		try {
			const params = new URLSearchParams({
				limit: String(pageSize),
				offset: String(page * pageSize)
			});
			// result: if not all selected, pick first (API is single-value for now;
			// multi-select handled client-side when all data fits in one page)
			if (selectedResults.length === 1) params.set('result', selectedResults[0]);
			if (selectedActions.length === 1) params.set('action', selectedActions[0]);
			if (selectedPolicyIds.length === 1) params.set('policy_id', selectedPolicyIds[0]);
			const since = sinceDate();
			if (since) params.set('since', since);

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

	// Client-side multi-filter on top of server results
	let filteredEvaluations = $derived(
		evaluations.filter((ev) => {
			if (selectedResults.length > 0 && !selectedResults.includes(ev.result)) return false;
			if (selectedActions.length > 0 && !selectedActions.includes(ev.action)) return false;
			if (selectedPolicyIds.length > 0 && ev.policy_id && !selectedPolicyIds.includes(ev.policy_id)) return false;
			return true;
		})
	);

	function setPage(p: number) {
		page = p;
		load();
	}

	function resetFilters() {
		selectedResults = ['fail'];
		selectedActions = ['warn'];
		selectedPolicyIds = [];
		selectedDays = 30;
		page = 0;
		load();
	}

	function isDefaultFilters() {
		return (
			selectedResults.length === 1 && selectedResults[0] === 'fail' &&
			selectedActions.length === 1 && selectedActions[0] === 'warn' &&
			selectedPolicyIds.length === 0 &&
			selectedDays === 30
		);
	}

	function toggleResult(r: string) {
		if (selectedResults.includes(r)) {
			selectedResults = selectedResults.filter((x) => x !== r);
		} else {
			selectedResults = [...selectedResults, r];
		}
		page = 0;
		load();
	}

	function toggleAction(a: string) {
		if (selectedActions.includes(a)) {
			selectedActions = selectedActions.filter((x) => x !== a);
		} else {
			selectedActions = [...selectedActions, a];
		}
		page = 0;
		load();
	}

	function togglePolicy(id: string) {
		if (selectedPolicyIds.includes(id)) {
			selectedPolicyIds = selectedPolicyIds.filter((x) => x !== id);
		} else {
			selectedPolicyIds = [...selectedPolicyIds, id];
		}
		page = 0;
		load();
	}

	function resultLabel(): string {
		if (selectedResults.length === 0 || selectedResults.length === ALL_RESULTS.length) return 'All results';
		return selectedResults.join(', ');
	}

	function actionLabel(): string {
		if (selectedActions.length === 0 || selectedActions.length === ALL_ACTIONS.length) return 'All actions';
		return selectedActions.map((a) => a === 'block_push' ? 'block' : a).join(', ');
	}

	function policyLabel(): string {
		if (selectedPolicyIds.length === 0) return 'All rules';
		if (selectedPolicyIds.length === 1) {
			return policies.find((p) => p.id === selectedPolicyIds[0])?.name ?? 'Unknown';
		}
		return `${selectedPolicyIds.length} rules`;
	}

	function dateLabel(): string {
		if (selectedDays === null) return 'All time';
		return DATE_PRESETS.find((p) => p.days === selectedDays)?.label ?? `${selectedDays}d`;
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

	$effect(() => { load(); });
</script>

<div class="border-border overflow-hidden rounded-lg border">
	<!-- Header + filters -->
	<div class="bg-muted/30 flex flex-wrap items-center justify-between gap-3 px-4 py-3">
		<span class="text-sm font-semibold">Policy Activity</span>
		<div class="flex flex-wrap items-center gap-2">

			<!-- Result filter -->
			<Popover.Root>
				<Popover.Trigger class="border-input bg-background hover:bg-accent hover:text-accent-foreground inline-flex h-8 items-center gap-1 rounded-md border px-3 text-xs font-medium transition-colors">
					{resultLabel()}
					{#if selectedResults.length > 0 && selectedResults.length < ALL_RESULTS.length}
						<span class="bg-primary text-primary-foreground ml-1 rounded-full px-1.5 py-0.5 text-[10px]">{selectedResults.length}</span>
					{/if}
					<ChevronDownIcon class="h-3 w-3 opacity-50" />
				</Popover.Trigger>
				<Popover.Content class="w-40 p-2" align="end">
					{#each ALL_RESULTS as r}
						<button class="flex w-full cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-xs hover:bg-muted" onclick={() => toggleResult(r)}>
							<span class="flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded-sm border border-input">
								{#if selectedResults.includes(r)}<CheckIcon class="h-3 w-3" />{/if}
							</span>
							<span class="rounded-full px-2 py-0.5 text-[10px]" style={resultPillStyle(r)}>{r}</span>
						</button>
					{/each}
					<div class="border-border mt-1 border-t pt-1">
						<button class="text-muted-foreground w-full rounded px-2 py-1 text-left text-xs hover:text-foreground"
							onclick={() => { selectedResults = [...ALL_RESULTS]; page = 0; load(); }}>
							Select all
						</button>
					</div>
				</Popover.Content>
			</Popover.Root>

			<!-- Action filter -->
			<Popover.Root>
				<Popover.Trigger class="border-input bg-background hover:bg-accent hover:text-accent-foreground inline-flex h-8 items-center gap-1 rounded-md border px-3 text-xs font-medium transition-colors">
					{actionLabel()}
					{#if selectedActions.length > 0 && selectedActions.length < ALL_ACTIONS.length}
						<span class="bg-primary text-primary-foreground ml-1 rounded-full px-1.5 py-0.5 text-[10px]">{selectedActions.length}</span>
					{/if}
					<ChevronDownIcon class="h-3 w-3 opacity-50" />
				</Popover.Trigger>
				<Popover.Content class="w-44 p-2" align="end">
					{#each ALL_ACTIONS as a}
						<button class="flex w-full cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-xs hover:bg-muted" onclick={() => toggleAction(a)}>
							<span class="flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded-sm border border-input">
								{#if selectedActions.includes(a)}<CheckIcon class="h-3 w-3" />{/if}
							</span>
							{#if a === 'block_push'}
								<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(240,101,101,0.08); color: #f06565">block</span>
							{:else}
								<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(246,177,68,0.08); color: #f6b144">warn</span>
							{/if}
						</button>
					{/each}
					<div class="border-border mt-1 border-t pt-1">
						<button class="text-muted-foreground w-full rounded px-2 py-1 text-left text-xs hover:text-foreground"
							onclick={() => { selectedActions = [...ALL_ACTIONS]; page = 0; load(); }}>
							Select all
						</button>
					</div>
				</Popover.Content>
			</Popover.Root>

			<!-- Policy filter -->
			{#if policies.length > 0}
				<Popover.Root>
					<Popover.Trigger class="border-input bg-background hover:bg-accent hover:text-accent-foreground inline-flex h-8 items-center gap-1 rounded-md border px-3 text-xs font-medium transition-colors">
						{policyLabel()}
						{#if selectedPolicyIds.length > 0}
							<span class="bg-primary text-primary-foreground ml-1 rounded-full px-1.5 py-0.5 text-[10px]">{selectedPolicyIds.length}</span>
						{/if}
						<ChevronDownIcon class="h-3 w-3 opacity-50" />
					</Popover.Trigger>
					<Popover.Content class="w-52 p-2" align="end">
						{#each policies as p}
							<button class="flex w-full cursor-pointer items-center gap-2 rounded px-2 py-1.5 text-xs hover:bg-muted" onclick={() => togglePolicy(p.id)}>
								<span class="flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded-sm border border-input">
									{#if selectedPolicyIds.includes(p.id)}<CheckIcon class="h-3 w-3" />{/if}
								</span>
								<span class="truncate">{p.name}</span>
							</button>
						{/each}
						<div class="border-border mt-1 border-t pt-1">
							<button class="text-muted-foreground w-full rounded px-2 py-1 text-left text-xs hover:text-foreground"
								onclick={() => { selectedPolicyIds = []; page = 0; load(); }}>
								All rules
							</button>
						</div>
					</Popover.Content>
				</Popover.Root>
			{/if}

			<!-- Date range filter -->
			<Popover.Root>
				<Popover.Trigger class="border-input bg-background hover:bg-accent hover:text-accent-foreground inline-flex h-8 items-center gap-1 rounded-md border px-3 text-xs font-medium transition-colors">
					{dateLabel()}
					<ChevronDownIcon class="h-3 w-3 opacity-50" />
				</Popover.Trigger>
				<Popover.Content class="w-44 p-2" align="end">
					{#each DATE_PRESETS as preset}
						<button
							class="w-full rounded px-2 py-1.5 text-left text-xs transition-colors {selectedDays === preset.days ? 'bg-primary text-primary-foreground' : 'hover:bg-muted'}"
							onclick={() => { selectedDays = preset.days; page = 0; load(); }}>
							{preset.label}
						</button>
					{/each}
				</Popover.Content>
			</Popover.Root>

			{#if !isDefaultFilters()}
				<Button size="sm" variant="ghost" class="h-8 text-xs text-muted-foreground" onclick={resetFilters}>
					Reset
				</Button>
			{/if}

			<Button size="sm" variant="outline" onclick={load} class="h-8 text-xs">Refresh</Button>
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
		{:else if filteredEvaluations.length === 0}
			<p class="text-muted-foreground text-sm">
				{#if evaluations.length === 0}
					No policy evaluations recorded yet. Run <code class="font-mono text-xs">tracevault check</code> from a repo with policies enabled to populate this view.
				{:else}
					No evaluations match the current filters.
					<button class="text-primary underline-offset-2 hover:underline" onclick={resetFilters}>Reset filters</button>
				{/if}
			</p>
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
						{#each filteredEvaluations as ev}
							<Table.Row class="hover:bg-muted/40 transition-colors">
								<Table.Cell class="text-xs whitespace-nowrap">{formatDate(ev.evaluated_at)}</Table.Cell>
								<Table.Cell class="text-xs font-medium">
									<span class="inline-flex items-center gap-1">
										{#if ev.is_synthetic}
											<ShieldIcon class="text-muted-foreground h-3 w-3 shrink-0" />
										{/if}
										{ev.policy_name}
										{#if ev.policy_id === null && !ev.is_synthetic}
											<span class="text-muted-foreground text-[10px]">(deleted)</span>
										{/if}
									</span>
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
