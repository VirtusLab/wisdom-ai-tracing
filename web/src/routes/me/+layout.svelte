<script lang="ts">
	import { page } from '$app/stores';
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

	// Personal "Profile" config tabs. Proxy config lives here as one section.
	const tabs = [
		{ href: '/me', label: 'Profile' },
		{ href: '/me/proxy', label: 'LLM Proxy' }
	];
	const pathname = $derived($page.url.pathname);
	function isActive(href: string): boolean {
		return href === '/me' ? pathname === '/me' : pathname.startsWith(href);
	}

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
		<div class="space-y-6">
			<h1 class="text-2xl font-bold">Profile</h1>
			<div class="flex gap-4 border-b pb-2 text-sm">
				{#each tabs as t}
					<a
						href={t.href}
						class={isActive(t.href)
							? 'font-semibold underline'
							: 'text-muted-foreground hover:underline'}
					>
						{t.label}
					</a>
				{/each}
			</div>
			{@render children()}
		</div>
	</AppLayout>
{:else}
	<div class="text-muted-foreground flex min-h-screen items-center justify-center text-sm">
		Loading...
	</div>
{/if}
