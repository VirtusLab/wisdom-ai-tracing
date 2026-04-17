<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { api } from '$lib/api';
	import { auth } from '$lib/stores/auth';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Alert from '$lib/components/ui/alert/index.js';
	import * as Select from '$lib/components/ui/select/index.js';
	import { BUILD_TIME } from '$lib/build-info';

	interface PublicOrg {
		name: string;
		display_name: string | null;
		sso_enabled: boolean;
		sso_enforce: boolean;
	}

	let email = $state('');
	let password = $state('');
	let error = $state('');
	let loading = $state(false);
	let ready = $state(false);
	let showPasswordForm = $state(false);

	let orgs = $state<PublicOrg[]>([]);
	let selectedSlug = $state('');

	let selectedOrg = $derived(orgs.find((o) => o.name === selectedSlug));

	onMount(async () => {
		const params = new URLSearchParams(window.location.search);
		const urlError = params.get('error');
		if (urlError) {
			error = urlError;
		}

		try {
			const feat = await api.get<{ initialized: boolean }>('/api/v1/features');
			if (!feat.initialized) {
				goto('/auth/setup');
				return;
			}
		} catch {
			// If features endpoint fails, show login anyway
		}

		try {
			orgs = await api.get<PublicOrg[]>('/api/v1/orgs/public');
			if (orgs.length === 1) {
				selectedSlug = orgs[0].name;
			}
		} catch {
			// If orgs endpoint fails, fall back to password login
			showPasswordForm = true;
		}

		ready = true;
	});

	function handleSsoLogin() {
		window.location.href = `/api/v1/auth/sso/${encodeURIComponent(selectedSlug)}`;
	}

	async function handleSubmit(e: Event) {
		e.preventDefault();
		error = '';
		loading = true;
		try {
			const resp = await api.post<{ token: string; user_id: string; email: string }>(
				'/api/v1/auth/login',
				{ email, password }
			);
			auth.setToken(resp.token);
			await auth.init();

			const params = new URLSearchParams(window.location.search);
			const redirect = params.get('redirect');
			goto(redirect || '/orgs');
		} catch (err) {
			error = err instanceof Error ? err.message : 'Login failed';
		} finally {
			loading = false;
		}
	}

	function orgLabel(org: PublicOrg): string {
		return org.display_name || org.name;
	}
</script>

<svelte:head>
	<title>Login - TraceVault</title>
</svelte:head>

{#if !ready}
	<div class="flex min-h-screen items-center justify-center">
		<p class="text-muted-foreground">Loading...</p>
	</div>
{:else}
	<div class="flex min-h-screen items-center justify-center">
		<div class="w-full max-w-md space-y-6">
			<div class="flex justify-center">
				<img src="/logo.png" alt="TraceVault" class="h-12 w-12 rounded-xl" />
			</div>
			<Card.Root>
			<Card.Header>
				<Card.Title class="text-2xl">Log in to TraceVault</Card.Title>
				<Card.Description>Select your organization to continue.</Card.Description>
			</Card.Header>
			<Card.Content>
				{#if error}
					<Alert.Root class="mb-4" variant="destructive">
						<Alert.Title>Error</Alert.Title>
						<Alert.Description>{error}</Alert.Description>
					</Alert.Root>
				{/if}

				<div class="grid gap-2 mb-4">
					<Label for="org">Organization</Label>
					<Select.Root type="single" value={selectedSlug} onValueChange={(v) => { if (v) { selectedSlug = v; showPasswordForm = false; } }}>
						<Select.Trigger id="org">
							{selectedOrg ? orgLabel(selectedOrg) : 'Select organization...'}
						</Select.Trigger>
						<Select.Content>
							{#each orgs as org}
								<Select.Item value={org.name}>{orgLabel(org)}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				{#if selectedOrg?.sso_enabled && !showPasswordForm}
					<Button class="w-full" onclick={handleSsoLogin}>
						Sign in with SSO
					</Button>

					{#if !selectedOrg.sso_enforce}
						<p class="mt-4 text-center text-sm text-muted-foreground">
							<button class="underline hover:text-foreground" onclick={() => showPasswordForm = true}>
								Sign in with password instead
							</button>
						</p>
					{/if}
				{:else if !selectedOrg?.sso_enabled || showPasswordForm}
					<form onsubmit={handleSubmit} class="grid gap-4">
						<div class="grid gap-2">
							<Label for="email">Email</Label>
							<Input id="email" type="email" bind:value={email} required placeholder="you@example.com" />
						</div>
						<div class="grid gap-2">
							<Label for="password">Password</Label>
							<Input id="password" type="password" bind:value={password} required />
						</div>
						<Button type="submit" class="w-full" disabled={loading}>
							{loading ? 'Logging in...' : 'Log in'}
						</Button>
					</form>

					{#if selectedOrg?.sso_enabled}
						<p class="mt-4 text-center text-sm text-muted-foreground">
							<button class="underline hover:text-foreground" onclick={() => showPasswordForm = false}>
								Sign in with SSO instead
							</button>
						</p>
					{/if}
				{/if}
			</Card.Content>
			<Card.Footer class="flex-col items-center gap-2">
				<p class="text-sm text-muted-foreground">
					Need access? <a href="/auth/setup" class="underline">Request an invitation</a>
				</p>
				<p class="text-[10px] text-muted-foreground/50">Built at: {BUILD_TIME}</p>
			</Card.Footer>
		</Card.Root>
		</div>
	</div>
{/if}
