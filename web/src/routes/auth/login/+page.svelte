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
	import { BUILD_TIME } from '$lib/build-info';

	let email = $state('');
	let password = $state('');
	let error = $state('');
	let loading = $state(false);
	let ready = $state(false);

	// SSO state
	let orgSlug = $state('');
	let ssoEnabled = $state(false);
	let ssoEnforce = $state(false);
	let ssoChecking = $state(false);
	let ssoCheckTimeout: ReturnType<typeof setTimeout> | null = null;

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
		ready = true;
	});

	function onOrgInput() {
		const val = orgSlug.trim().toLowerCase();
		if (ssoCheckTimeout) clearTimeout(ssoCheckTimeout);
		ssoEnabled = false;
		ssoEnforce = false;
		if (val.length < 2) return;
		ssoChecking = true;
		ssoCheckTimeout = setTimeout(() => checkSsoStatus(val), 500);
	}

	async function checkSsoStatus(slug: string) {
		try {
			const resp = await api.get<{ sso_enabled: boolean; enforce: boolean }>(
				`/api/v1/auth/sso-status/${encodeURIComponent(slug)}`
			);
			ssoEnabled = resp.sso_enabled;
			ssoEnforce = resp.enforce;
		} catch {
			ssoEnabled = false;
			ssoEnforce = false;
		} finally {
			ssoChecking = false;
		}
	}

	function handleSsoLogin() {
		const slug = orgSlug.trim().toLowerCase();
		window.location.href = `/api/v1/auth/sso/${encodeURIComponent(slug)}`;
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
				<Card.Description>Enter your credentials to continue.</Card.Description>
			</Card.Header>
			<Card.Content>
				{#if error}
					<Alert.Root class="mb-4" variant="destructive">
						<Alert.Title>Error</Alert.Title>
						<Alert.Description>{error}</Alert.Description>
					</Alert.Root>
				{/if}

				<!-- Organization field for SSO -->
				<div class="grid gap-2 mb-4">
					<Label for="org">Organization</Label>
					<Input
						id="org"
						type="text"
						bind:value={orgSlug}
						oninput={onOrgInput}
						placeholder="your-org-slug"
					/>
					{#if ssoChecking}
						<p class="text-xs text-muted-foreground">Checking SSO...</p>
					{/if}
				</div>

				{#if ssoEnabled}
					<div class="mb-4">
						<Button class="w-full" onclick={handleSsoLogin}>
							Sign in with SSO
						</Button>
					</div>
				{/if}

				{#if ssoEnabled && ssoEnforce}
					<p class="text-xs text-muted-foreground text-center">
						This organization requires SSO login.
					</p>
				{:else}
					{#if ssoEnabled}
						<div class="relative my-4">
							<div class="absolute inset-0 flex items-center">
								<span class="w-full border-t"></span>
							</div>
							<div class="relative flex justify-center text-xs uppercase">
								<span class="bg-card px-2 text-muted-foreground">or</span>
							</div>
						</div>
					{/if}

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
