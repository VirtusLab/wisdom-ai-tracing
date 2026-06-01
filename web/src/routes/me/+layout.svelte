<script lang="ts">
	import { orgStore } from '$lib/stores/org';
	import AppLayout from '$lib/components/app-layout.svelte';

	interface OrgInfo {
		org_id: string;
		org_name: string;
		display_name: string | null;
		role: string;
	}

	let { children } = $props();

	let loaded = $state(false);

	$effect(() => {
		loadIfNeeded();
	});

	async function loadIfNeeded() {
		// Pull the user's org list so the sidebar (Org switcher, nav links
		// like /orgs/{slug}/dashboard) renders with a real slug — the
		// /me/* routes aren't org-scoped themselves, but the chrome around
		// them is. If a current org is already set in the store from a
		// previous navigation we just use that; otherwise default to the
		// first membership.
		const orgs: OrgInfo[] = await orgStore.loadOrgs();
		// Read the current store snapshot. svelte/store invokes the
		// subscriber synchronously with the current value on `.subscribe()`,
		// so we cannot call `unsub` from inside that first callback —
		// at that point `unsub` is still in its TDZ. Stash the unsubscribe
		// in an outer binding and call it after the Promise settles.
		let unsub: (() => void) | undefined;
		const state = await new Promise<{ current: OrgInfo | null }>((resolve) => {
			unsub = orgStore.subscribe((s) => resolve(s));
		});
		unsub?.();
		if (!state.current && orgs.length > 0) {
			orgStore.setCurrent(orgs[0]);
		}
		loaded = true;
	}
</script>

{#if loaded}
	<AppLayout>
		{@render children()}
	</AppLayout>
{:else}
	<div class="text-muted-foreground flex min-h-screen items-center justify-center text-sm">
		Loading...
	</div>
{/if}
