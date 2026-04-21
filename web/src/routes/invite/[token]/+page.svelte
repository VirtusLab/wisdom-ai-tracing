<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { api, ApiError } from '$lib/api';
	import { auth } from '$lib/stores/auth';
	import { Button } from '$lib/components/ui/button/index.js';
	import * as Card from '$lib/components/ui/card/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import * as Alert from '$lib/components/ui/alert/index.js';
	import PasswordStrength from '$lib/components/password-strength.svelte';
	import { formatDateTime } from '$lib/utils/date';

	interface InviteDetails {
		email: string;
		org_name: string;
		org_slug: string;
		existing_user: boolean;
		expires_at: string;
	}

	const token = $derived($page.params.token);

	let authState: { isAuthenticated: boolean; loading: boolean } = $state({
		isAuthenticated: false,
		loading: true
	});
	auth.subscribe((s) => (authState = s));

	let invite: InviteDetails | null = $state(null);
	let loading = $state(true);
	let error = $state('');

	// New user signup form
	let password = $state('');
	let name = $state('');
	let submitLoading = $state(false);
	let submitError = $state('');
	let passwordStrength: PasswordStrength | null = $state(null);

	// Existing user accept
	let acceptLoading = $state(false);
	let acceptError = $state('');

	onMount(async () => {
		try {
			invite = await api.get<InviteDetails>(`/api/v1/invite/${token}`);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Invite not found or expired';
		} finally {
			loading = false;
		}
	});

	async function handleSignup(e: Event) {
		e.preventDefault();
		submitError = '';
		if (passwordStrength && !passwordStrength.isStrong()) {
			submitError = 'Please choose a stronger password.';
			return;
		}
		submitLoading = true;
		try {
			const res = await api.post<{ token: string; user_id: string; email: string; org_name: string }>(
				`/api/v1/invite/${token}/accept`,
				{ password, name: name || undefined }
			);
			auth.setToken(res.token);
			await auth.init();
			goto(`/orgs/${invite!.org_slug}/repos`);
		} catch (err) {
			if (err instanceof ApiError && err.code === 'account_exists') {
				goto(`/auth/login?redirect=/invite/${token}`);
				return;
			}
			submitError = err instanceof Error ? err.message : 'Failed to create account';
		} finally {
			submitLoading = false;
		}
	}

	async function handleAcceptExisting() {
		acceptError = '';
		acceptLoading = true;
		try {
			await api.post(`/api/v1/invite/${token}/accept/existing`);
			goto(`/orgs/${invite!.org_slug}/repos`);
		} catch (err) {
			acceptError = err instanceof Error ? err.message : 'Failed to accept invite';
		} finally {
			acceptLoading = false;
		}
	}
</script>

<svelte:head>
	<title>Accept Invite - TraceVault</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center">
	<div class="w-full max-w-md space-y-6">
		<div class="flex justify-center">
			<img src="/logo.png" alt="TraceVault" class="h-12 w-12 rounded-xl" />
		</div>

		{#if loading}
			<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm">
				<span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"></span>
				Loading invite...
			</div>
		{:else if error}
			<Card.Root>
				<Card.Header>
					<Card.Title class="text-2xl">Invalid Invite</Card.Title>
				</Card.Header>
				<Card.Content>
					<Alert.Root variant="destructive">
						<Alert.Description>{error}</Alert.Description>
					</Alert.Root>
				</Card.Content>
				<Card.Footer class="justify-center">
					<p class="text-sm text-muted-foreground">
						<a href="/auth/login" class="underline">Go to login</a>
					</p>
				</Card.Footer>
			</Card.Root>
		{:else if invite}
			<Card.Root>
				<Card.Header>
					<Card.Title class="text-2xl">Join {invite.org_name}</Card.Title>
					<Card.Description>
						You've been invited to join <strong>{invite.org_name}</strong> as <strong>{invite.email}</strong>.
					</Card.Description>
				</Card.Header>
				<Card.Content>
					<p class="text-xs text-muted-foreground mb-4">Expires {formatDateTime(invite.expires_at)}</p>

					{#if invite.existing_user && authState.isAuthenticated}
						<!-- Logged-in existing user: one-click accept -->
						{#if acceptError}
							<Alert.Root variant="destructive" class="mb-4">
								<Alert.Description>{acceptError}</Alert.Description>
							</Alert.Root>
						{/if}
						<Button class="w-full" disabled={acceptLoading} onclick={handleAcceptExisting}>
							{acceptLoading ? 'Joining...' : 'Accept invite'}
						</Button>
					{:else if invite.existing_user}
						<!-- Existing user but not logged in: redirect to login -->
						<p class="text-sm text-muted-foreground mb-4">
							An account with this email already exists. Log in to accept this invite.
						</p>
						<Button class="w-full" onclick={() => goto(`/auth/login?redirect=/invite/${token}`)}>
							Log in to accept
						</Button>
					{:else}
						<!-- New user: signup form -->
						{#if submitError}
							<Alert.Root variant="destructive" class="mb-4">
								<Alert.Description>{submitError}</Alert.Description>
							</Alert.Root>
						{/if}
						<form onsubmit={handleSignup} class="grid gap-4">
							<div class="grid gap-2">
								<Label for="invite_name">Your name</Label>
								<Input id="invite_name" bind:value={name} placeholder="Jane Doe" />
							</div>
							<div class="grid gap-2">
								<Label for="invite_password">Password</Label>
								<Input id="invite_password" type="password" bind:value={password} required minlength={10} />
								<PasswordStrength {password} bind:this={passwordStrength} />
							</div>
							<Button type="submit" class="w-full" disabled={submitLoading}>
								{submitLoading ? 'Creating account...' : 'Create account & join'}
							</Button>
						</form>
					{/if}
				</Card.Content>
				<Card.Footer class="justify-center">
					<p class="text-sm text-muted-foreground">
						Already have an account? <a href="/auth/login" class="underline">Log in</a>
					</p>
				</Card.Footer>
			</Card.Root>
		{/if}
	</div>
</div>
