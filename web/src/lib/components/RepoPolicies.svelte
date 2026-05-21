<script lang="ts">
	import * as Table from '$lib/components/ui/table/index.js';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import * as Select from '$lib/components/ui/select/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import { api } from '$lib/api';

	interface Policy {
		id: string;
		org_id: string;
		repo_id: string | null;
		name: string;
		description: string;
		condition: Record<string, unknown>;
		action: string;
		severity: string;
		scope: string;
		enabled: boolean;
		created_at: string;
		updated_at: string;
	}

	interface Props {
		slug: string;
		repoId: string;
		policies: Policy[];
		loading: boolean;
		error: string;
		onchange: () => void;
	}

	let { slug, repoId, policies, loading, error, onchange }: Props = $props();

	// Create dialog state
	let createOpen = $state(false);
	let createLoading = $state(false);
	let createError = $state('');
	let newName = $state('');
	let newDescription = $state('');
	let newConditionType = $state('ConditionalToolCall');
	let newToolName = $state('');
	let newMinCount = $state('1');
	let newFilePatterns = $state('');
	let newAction = $state('block_push');
	let newScope = $state('session');
	let newToolNames = $state('');
	let newMustSucceed = $state(false);

	function buildCondition(): Record<string, unknown> {
		if (newConditionType === 'RequiredToolCall') {
			return {
				type: 'RequiredToolCall',
				tool_names: newToolNames
					.split(',')
					.map((s) => s.trim())
					.filter((s) => s.length > 0),
				must_succeed: newMustSucceed
			};
		} else {
			const condition: Record<string, unknown> = {
				type: 'ConditionalToolCall',
				tool_name: newToolName,
				min_count: parseInt(newMinCount) || 1,
				must_succeed: newMustSucceed
			};
			const patterns = newFilePatterns
				.split(',')
				.map((s) => s.trim())
				.filter((s) => s.length > 0);
			if (patterns.length > 0) {
				condition.when_files_match = patterns;
			}
			return condition;
		}
	}

	async function handleCreate(e: Event) {
		e.preventDefault();
		createLoading = true;
		createError = '';
		try {
			await api.post(`/api/v1/orgs/${slug}/repos/${repoId}/policies`, {
				name: newName,
				description: newDescription || undefined,
				condition: buildCondition(),
				action: newAction,
				scope: newScope
			});
			createOpen = false;
			resetForm();
			onchange();
		} catch (err) {
			createError = err instanceof Error ? err.message : 'Failed to create policy';
		} finally {
			createLoading = false;
		}
	}

	function resetForm() {
		newName = '';
		newDescription = '';
		newConditionType = 'ConditionalToolCall';
		newToolName = '';
		newMinCount = '1';
		newFilePatterns = '';
		newToolNames = '';
		newMustSucceed = false;
		newAction = 'block_push';
		newScope = 'session';
		createError = '';
	}

	async function togglePolicy(policy: Policy) {
		try {
			await api.put(`/api/v1/orgs/${slug}/policies/${policy.id}`, {
				enabled: !policy.enabled
			});
			onchange();
		} catch {
			// error surfaced via parent reload
		}
	}

	async function deletePolicy(id: string) {
		if (!confirm('Delete this policy? This cannot be undone.')) return;
		try {
			await api.delete(`/api/v1/orgs/${slug}/policies/${id}`);
			onchange();
		} catch {
			// error surfaced via parent reload
		}
	}

	function conditionSummary(condition: Record<string, unknown>): string {
		const type = condition.type as string;
		const mustSucceed = condition.must_succeed === true;
		if (type === 'RequiredToolCall') {
			const tools = (condition.tool_names as string[]) || [];
			return `Require: ${tools.join(', ')}${mustSucceed ? ' ✓ must succeed' : ''}`;
		} else if (type === 'ConditionalToolCall') {
			const tool = condition.tool_name as string;
			const min = (condition.min_count as number) || 1;
			const patterns = condition.when_files_match as string[] | undefined;
			let s = `${tool} >= ${min}x`;
			if (patterns && patterns.length > 0) s += ` when ${patterns.join(', ')}`;
			if (mustSucceed) s += ' ✓ must succeed';
			return s;
		}
		return JSON.stringify(condition);
	}
</script>

<div class="border-border overflow-hidden rounded-lg border">
	<div class="bg-muted/30 flex items-center justify-between px-4 py-3">
		<span class="text-sm font-semibold">Policies</span>
		<Dialog.Root bind:open={createOpen} onOpenChange={(open) => { if (!open) resetForm(); }}>
			<Dialog.Trigger>
				{#snippet child({ props })}
					<Button size="sm" {...props}>Add Policy</Button>
				{/snippet}
			</Dialog.Trigger>
			<Dialog.Content class="sm:max-w-lg">
				<Dialog.Header>
					<Dialog.Title>Create Policy</Dialog.Title>
					<Dialog.Description>Define a tool-call requirement for this repo.</Dialog.Description>
				</Dialog.Header>
				<form onsubmit={handleCreate} class="grid gap-4">
					{#if createError}
						<p class="text-sm text-destructive">{createError}</p>
					{/if}
					<div class="grid gap-2">
						<Label for="policy_name">Name</Label>
						<Input id="policy_name" bind:value={newName} required placeholder="e.g., Code review required" />
					</div>
					<div class="grid gap-2">
						<Label for="policy_desc">Description</Label>
						<Input id="policy_desc" bind:value={newDescription} placeholder="Optional description" />
					</div>
					<div class="grid gap-2">
						<Label>Condition Type</Label>
						<Select.Root type="single" value={newConditionType} onValueChange={(v) => { if (v) newConditionType = v; }}>
							<Select.Trigger>{newConditionType === 'ConditionalToolCall' ? 'Conditional Tool Call' : 'Required Tool Call'}</Select.Trigger>
							<Select.Content>
								<Select.Item value="ConditionalToolCall">Conditional Tool Call</Select.Item>
								<Select.Item value="RequiredToolCall">Required Tool Call</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>

					{#if newConditionType === 'ConditionalToolCall'}
						<div class="grid gap-2">
							<Label for="tool_name">Tool Name</Label>
							<Input id="tool_name" bind:value={newToolName} required placeholder="e.g., mcp__cargo__cargo_check" />
						</div>
						<div class="grid gap-2">
							<Label for="min_count">Minimum Calls</Label>
							<Input id="min_count" type="number" min="1" bind:value={newMinCount} />
						</div>
						<div class="grid gap-2">
							<Label for="file_patterns">File Patterns (comma-separated, optional)</Label>
							<Input id="file_patterns" bind:value={newFilePatterns} placeholder="e.g., src/**/*.rs, Cargo.lock" />
						</div>
					{:else}
						<div class="grid gap-2">
							<Label for="tool_names">Tool Names (comma-separated)</Label>
							<Input id="tool_names" bind:value={newToolNames} required placeholder="e.g., mcp__cargo__cargo_fmt, Bash" />
						</div>
					{/if}

					<div class="flex items-center gap-2">
						<input
							type="checkbox"
							id="must_succeed"
							bind:checked={newMustSucceed}
							class="h-4 w-4 rounded border-border accent-primary"
						/>
						<Label for="must_succeed" class="cursor-pointer">
							Must succeed
							<span class="text-muted-foreground text-xs font-normal ml-1">(only count calls where the tool did not return an error)</span>
						</Label>
					</div>

					<div class="grid gap-2">
						<Label>Action</Label>
						<Select.Root type="single" value={newAction} onValueChange={(v) => { if (v) newAction = v; }}>
							<Select.Trigger>{{ block_push: 'Block Push', warn: 'Warn', allow: 'Allow' }[newAction] ?? newAction}</Select.Trigger>
							<Select.Content>
								<Select.Item value="block_push">Block Push</Select.Item>
								<Select.Item value="warn">Warn</Select.Item>
								<Select.Item value="allow">Allow (permitted in validation window, no count required)</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>

					<div class="grid gap-2">
						<Label>Scope</Label>
						<Select.Root type="single" value={newScope} onValueChange={(v) => { if (v) newScope = v; }}>
							<Select.Trigger>{newScope === 'session' ? 'Session (whole push)' : newScope === 'validation_window' ? 'Validation Window only' : 'Both'}</Select.Trigger>
							<Select.Content>
								<Select.Item value="session">Session — evaluated over entire push window (default)</Select.Item>
								<Select.Item value="validation_window">Validation Window — evaluated only inside declared window</Select.Item>
								<Select.Item value="both">Both — evaluated in session and validation window</Select.Item>
							</Select.Content>
						</Select.Root>
						<p class="text-xs text-muted-foreground">Use <em>Validation Window</em> with <code>tracevault validation-start</code> to enforce checks run after code changes.</p>
					</div>

					<Dialog.Footer>
						<Button type="submit" disabled={createLoading}>
							{createLoading ? 'Creating...' : 'Create'}
						</Button>
					</Dialog.Footer>
				</form>
			</Dialog.Content>
		</Dialog.Root>
	</div>
	<div class="p-4">
		{#if loading}
			<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm">
				<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"></span>
				Loading...
			</div>
		{:else if error}
			<p class="text-destructive">{error}</p>
		{:else if policies.length === 0}
			<p class="text-muted-foreground text-sm">No policies configured. Add a policy to enforce tool-call requirements.</p>
		{:else}
			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head class="text-xs">Name</Table.Head>
						<Table.Head class="text-xs">Condition</Table.Head>
						<Table.Head class="text-xs">Action</Table.Head>
						<Table.Head class="text-xs">Scope</Table.Head>
						<Table.Head class="text-xs">Enabled</Table.Head>
						<Table.Head class="text-xs"></Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each policies as policy}
						<Table.Row class="hover:bg-muted/40 transition-colors">
							<Table.Cell class="text-xs font-medium">{policy.name}</Table.Cell>
							<Table.Cell class="font-mono text-xs max-w-xs truncate">{conditionSummary(policy.condition)}</Table.Cell>
							<Table.Cell class="text-xs">
								{#if policy.action === 'block_push'}
									<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(240,101,101,0.12); color: #f06565; border: 1px solid rgba(240,101,101,0.25)">Block</span>
								{:else if policy.action === 'allow'}
									<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(52,211,153,0.12); color: #34d399; border: 1px solid rgba(52,211,153,0.25)">Allow</span>
								{:else}
									<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(246,177,68,0.12); color: #f6b144; border: 1px solid rgba(246,177,68,0.25)">Warn</span>
								{/if}
							</Table.Cell>
							<Table.Cell class="text-xs">
								{#if policy.scope === 'validation_window'}
									<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(99,179,237,0.12); color: #63b3ed; border: 1px solid rgba(99,179,237,0.25)">Window</span>
								{:else if policy.scope === 'both'}
									<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(167,139,250,0.12); color: #a78bfa; border: 1px solid rgba(167,139,250,0.25)">Both</span>
								{:else}
									<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(156,163,175,0.12); color: #9ca3af; border: 1px solid rgba(156,163,175,0.25)">Session</span>
								{/if}
							</Table.Cell>
							<Table.Cell class="text-xs">
								<Button variant="ghost" size="sm" onclick={() => togglePolicy(policy)}>
									{policy.enabled ? 'On' : 'Off'}
								</Button>
							</Table.Cell>
							<Table.Cell class="text-xs">
								{#if policy.repo_id}
									<Button variant="destructive" size="sm" onclick={() => deletePolicy(policy.id)}>
										Delete
									</Button>
								{/if}
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
		{/if}
	</div>
</div>
