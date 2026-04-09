<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { auth } from '$lib/stores/auth';

	onMount(() => {
		const hash = window.location.hash.substring(1);
		const params = new URLSearchParams(hash);
		const token = params.get('token');
		const org = params.get('org');

		// Clear the fragment immediately
		history.replaceState(null, '', window.location.pathname);

		if (!token) {
			goto('/auth/login?error=SSO+authentication+failed');
			return;
		}

		auth.setToken(token);
		auth.init().then(() => {
			goto(org ? `/orgs/${org}/repos` : '/orgs');
		});
	});
</script>

<svelte:head>
	<title>Completing sign-in... - TraceVault</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center">
	<p class="text-muted-foreground">Completing sign-in...</p>
</div>
