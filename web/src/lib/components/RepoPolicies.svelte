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

	// Dialog state — shared between create and edit modes.
	let dialogOpen = $state(false);
	let dialogLoading = $state(false);
	let dialogError = $state('');
	// When set, the dialog is in edit mode for this policy.
	let editingPolicy = $state<Policy | null>(null);

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

	const isEdit = $derived(editingPolicy !== null);
	const dialogTitle = $derived(isEdit ? 'Edit Policy' : 'Create Policy');
	const dialogDescription = $derived(
		isEdit
			? 'Update this policy. Only changed fields are saved.'
			: 'Define a tool-call requirement for this repo.'
	);

	function getSubmitLabel(loading: boolean, editing: boolean): string {
		if (loading && editing) return 'Saving...';
		if (loading) return 'Creating...';
		if (editing) return 'Save';
		return 'Create';
	}

	const submitLabel = $derived(getSubmitLabel(dialogLoading, isEdit));

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

	async function handleSubmit(e: Event) {
		e.preventDefault();
		dialogLoading = true;
		dialogError = '';
		try {
			const payload = {
				name: newName,
				description: newDescription || undefined,
				condition: buildCondition(),
				action: newAction,
				scope: newScope
			};
			if (editingPolicy) {
				await api.put(`/api/v1/orgs/${slug}/policies/${editingPolicy.id}`, payload);
			} else {
				await api.post(`/api/v1/orgs/${slug}/repos/${repoId}/policies`, payload);
			}
			dialogOpen = false;
			resetForm();
			onchange();
		} catch (err) {
			dialogError =
				err instanceof Error
					? err.message
					: editingPolicy
						? 'Failed to update policy'
						: 'Failed to create policy';
		} finally {
			dialogLoading = false;
		}
	}

	function resetForm() {
		editingPolicy = null;
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
		dialogError = '';
	}

	function openCreate() {
		resetForm();
		dialogOpen = true;
	}

	function openEdit(policy: Policy) {
		resetForm();
		editingPolicy = policy;
		newName = policy.name;
		newDescription = policy.description ?? '';
		newAction = policy.action;
		newScope = policy.scope;
		const cond = policy.condition;
		const type = cond.type as string;
		newConditionType = type === 'RequiredToolCall' ? 'RequiredToolCall' : 'ConditionalToolCall';
		newMustSucceed = cond.must_succeed === true;
		if (type === 'RequiredToolCall') {
			const tools = (cond.tool_names as string[]) ?? [];
			newToolNames = tools.join(', ');
		} else {
			newToolName = (cond.tool_name as string) ?? '';
			newMinCount = String((cond.min_count as number) ?? 1);
			const patterns = (cond.when_files_match as string[] | undefined) ?? [];
			newFilePatterns = patterns.join(', ');
		}
		dialogOpen = true;
	}

	let actionError = $state('');

	async function togglePolicy(policy: Policy) {
		actionError = '';
		try {
			await api.put(`/api/v1/orgs/${slug}/policies/${policy.id}`, {
				enabled: !policy.enabled
			});
			onchange();
		} catch (err) {
			actionError = err instanceof Error ? err.message : 'Failed to update policy';
		}
	}

	async function deletePolicy(id: string) {
		if (!confirm('Delete this policy? This cannot be undone.')) return;
		actionError = '';
		try {
			await api.delete(`/api/v1/orgs/${slug}/policies/${id}`);
			onchange();
		} catch (err) {
			actionError = err instanceof Error ? err.message : 'Failed to delete policy';
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
		<Button size="sm" onclick={openCreate}>Add Policy</Button>
		<Dialog.Root bind:open={dialogOpen} onOpenChange={(open) => { if (!open) resetForm(); }}>
			<Dialog.Content class="sm:max-w-lg">
				<Dialog.Header>
					<Dialog.Title>{dialogTitle}</Dialog.Title>
					<Dialog.Description>{dialogDescription}</Dialog.Description>
				</Dialog.Header>
				<form onsubmit={handleSubmit} class="grid gap-4">
					{#if dialogError}
						<p class="text-sm text-destructive">{dialogError}</p>
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
								<Select.Item value="allow">Allow</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>

					<div class="grid gap-2">
						<Label>Scope</Label>
						<Select.Root type="single" value={newScope} onValueChange={(v) => { if (v) newScope = v; }}>
							<Select.Trigger>{{ session: 'Session', validation_window: 'Validation', both: 'Session' }[newScope] ?? newScope}</Select.Trigger>
							<Select.Content>
								<Select.Item value="session">Session</Select.Item>
								<Select.Item value="validation_window">Validation</Select.Item>
							</Select.Content>
						</Select.Root>
					</div>

					<Dialog.Footer>
						<Button type="submit" disabled={dialogLoading}>{submitLabel}</Button>
					</Dialog.Footer>
				</form>
			</Dialog.Content>
		</Dialog.Root>
	</div>
	<div class="p-4">
		{#if actionError}
			<p class="text-destructive text-sm mb-2">{actionError}</p>
		{/if}
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
									<span class="rounded-full px-2 py-0.5 text-[10px]" style="background: rgba(99,179,237,0.12); color: #63b3ed; border: 1px solid rgba(99,179,237,0.25)">Validation</span>

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
									<div class="flex justify-end gap-2">
										<Button variant="outline" size="sm" onclick={() => openEdit(policy)}>Edit</Button>
										<Button
											variant="destructive"
											size="sm"
											onclick={() => deletePolicy(policy.id)}
										>
											Delete
										</Button>
									</div>
								{/if}
							</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
		{/if}
	</div>
</div>
