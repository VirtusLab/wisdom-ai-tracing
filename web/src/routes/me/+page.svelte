<script lang="ts">
	import { api } from '$lib/api';
	import ErrorState from '$lib/components/ErrorState.svelte';

	interface Me {
		user_id: string;
		email: string;
		name: string | null;
	}

	interface OrgMembership {
		org_id: string;
		org_name: string;
		display_name: string | null;
		role: string;
	}

	let me = $state<Me | null>(null);
	let orgs = $state<OrgMembership[]>([]);
	let error = $state('');
	let loading = $state(true);

	$effect(() => {
		load();
	});

	async function load() {
		loading = true;
		error = '';
		try {
			const [m, o] = await Promise.all([
				api.get<Me>('/api/v1/auth/me'),
				api.get<OrgMembership[]>('/api/v1/me/orgs')
			]);
			me = m;
			orgs = o;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load profile';
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Profile - TraceVault</title>
</svelte:head>

{#if error}
	<ErrorState message={error} />
{:else if loading}
	<p class="text-muted-foreground text-sm">Loading...</p>
{:else if me}
	<div class="border-border max-w-lg overflow-hidden rounded-lg border">
		<div class="bg-muted/30 px-4 py-3 text-sm font-semibold">Account</div>
		<div class="space-y-3 p-4 text-sm">
			<div class="flex items-center justify-between gap-4">
				<span class="text-muted-foreground">Email</span>
				<span class="font-mono">{me.email}</span>
			</div>
			<div class="flex items-center justify-between gap-4">
				<span class="text-muted-foreground">Name</span>
				<span>{me.name || '—'}</span>
			</div>
		</div>
	</div>

	<div class="border-border max-w-lg overflow-hidden rounded-lg border">
		<div class="bg-muted/30 px-4 py-3 text-sm font-semibold">
			Organizations ({orgs.length})
		</div>
		{#if orgs.length === 0}
			<p class="text-muted-foreground p-4 text-sm">No organization memberships.</p>
		{:else}
			<div class="divide-border divide-y">
				{#each orgs as org (org.org_id)}
					<div class="flex items-center justify-between gap-4 px-4 py-3 text-sm">
						<a href="/orgs/{org.org_name}/dashboard" class="hover:underline">
							{org.display_name || org.org_name}
						</a>
						<span class="text-muted-foreground text-xs capitalize">{org.role}</span>
					</div>
				{/each}
			</div>
		{/if}
	</div>
{/if}
