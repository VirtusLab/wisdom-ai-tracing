<script lang="ts">
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { api } from '$lib/api';
	import { orgStore } from '$lib/stores/org';
	import { features } from '$lib/stores/features';
	import EnterpriseUpgrade from '$lib/components/enterprise-upgrade.svelte';
	import * as Select from '$lib/components/ui/select/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Input } from '$lib/components/ui/input/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import ErrorState from '$lib/components/ErrorState.svelte';

	interface ComplianceSettings {
		org_id: string;
		retention_days: number;
		signing_enabled: boolean;
		compliance_mode: string;
		chain_verification_interval_hours: number | null;
	}

	const slug = $derived($page.params.slug);

	let orgState: { current: { role: string } | null } = $state({ current: null });
	orgStore.subscribe((s) => (orgState = s));

	let settings: ComplianceSettings | null = $state(null);
	let loading = $state(true);
	let saving = $state(false);
	let error = $state('');
	let success = $state('');

	// Form state
	let complianceMode = $state('none');
	let retentionDays = $state('365');
	let signingEnabled = $state(false);
	let verificationHours = $state('24');

	const canEdit = $derived(
		orgState.current?.role === 'owner' || orgState.current?.role === 'admin'
	);

	onMount(async () => {
		await loadSettings();
	});

	async function loadSettings() {
		loading = true;
		try {
			settings = await api.get<ComplianceSettings>(`/api/v1/orgs/${slug}/compliance`);
			complianceMode = settings.compliance_mode;
			retentionDays = String(settings.retention_days);
			signingEnabled = settings.signing_enabled;
			verificationHours = String(settings.chain_verification_interval_hours ?? 24);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load settings';
		} finally {
			loading = false;
		}
	}

	async function handleSave(e: Event) {
		e.preventDefault();
		saving = true;
		error = '';
		success = '';
		try {
			settings = await api.put<ComplianceSettings>(`/api/v1/orgs/${slug}/compliance`, {
				compliance_mode: complianceMode,
				retention_days: parseInt(retentionDays) || 365,
				signing_enabled: signingEnabled,
				chain_verification_interval_hours: parseInt(verificationHours) || 24
			});
			success = 'Settings saved successfully.';
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to save settings';
		} finally {
			saving = false;
		}
	}

	const modeDescriptions: Record<string, string> = {
		none: 'No compliance requirements enforced.',
		sox: 'SOX: 7-year retention, signing required, daily chain verification, strict SoD.',
		pci_dss: 'PCI-DSS: 1-year retention, signing required, daily chain verification, strict SoD.',
		sr_11_7: 'SR 11-7: 3-year retention, signing required, weekly chain verification.',
		custom: 'Custom: Configure each setting individually.'
	};

	const minRetention: Record<string, number> = {
		none: 0,
		sox: 2555,
		pci_dss: 365,
		sr_11_7: 1095,
		custom: 0
	};
</script>

<svelte:head>
	<title>Compliance Settings - TraceVault</title>
</svelte:head>

{#if !$features.loaded}
	<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm"><span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"></span>Loading...</div>
{:else if $features.compliance}
<div class="space-y-6 max-w-2xl">
	<div class="flex items-center gap-2">
		<a href="/orgs/{slug}/compliance" class="text-muted-foreground hover:underline">Compliance</a>
		<span class="text-muted-foreground">/</span>
		<h1 class="text-2xl font-bold">Settings</h1>
	</div>

	{#if loading}
		<div class="text-muted-foreground flex items-center justify-center gap-2 py-12 text-sm"><span class="inline-block h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent"></span>Loading...</div>
	{:else if error && !settings}
		<ErrorState message={error} />
	{:else}
		<div class="border-border overflow-hidden rounded-lg border">
			<div class="bg-muted/30 px-4 py-3 text-sm font-semibold">Compliance Configuration</div>
			<div class="p-4 space-y-3">
				{#if error}
					<p class="text-sm text-destructive mb-4">{error}</p>
				{/if}
				{#if success}
					<p class="text-sm text-green-600 mb-4">{success}</p>
				{/if}

				<form onsubmit={handleSave} class="grid gap-6">
					<div class="grid gap-2">
						<Label>Compliance Mode</Label>
						<Select.Root
							type="single"
							value={complianceMode}
							onValueChange={(v) => {
								if (v) complianceMode = v;
							}}
							disabled={!canEdit}
						>
							<Select.Trigger>
								{complianceMode === 'none'
									? 'None'
									: complianceMode === 'sox'
										? 'SOX'
										: complianceMode === 'pci_dss'
											? 'PCI-DSS'
											: complianceMode === 'sr_11_7'
												? 'SR 11-7'
												: 'Custom'}
							</Select.Trigger>
							<Select.Content>
								<Select.Item value="none">None</Select.Item>
								<Select.Item value="sox">SOX</Select.Item>
								<Select.Item value="pci_dss">PCI-DSS</Select.Item>
								<Select.Item value="sr_11_7">SR 11-7</Select.Item>
								<Select.Item value="custom">Custom</Select.Item>
							</Select.Content>
						</Select.Root>
						<p class="text-xs text-muted-foreground">{modeDescriptions[complianceMode]}</p>
					</div>

					<div class="grid gap-2">
						<Label for="retention">Retention Period (days)</Label>
						<Input
							id="retention"
							type="number"
							min={minRetention[complianceMode] || 1}
							bind:value={retentionDays}
							disabled={!canEdit}
						/>
						{#if minRetention[complianceMode] > 0}
							<p class="text-xs text-muted-foreground">
								Minimum {minRetention[complianceMode]} days required for {complianceMode}
								mode
							</p>
						{/if}
					</div>

					<div class="grid gap-2">
						<Label>Trace Signing</Label>
						<div class="flex items-center gap-2">
							<Button
								type="button"
								variant={signingEnabled ? 'default' : 'outline'}
								size="sm"
								onclick={() => {
									if (canEdit) signingEnabled = !signingEnabled;
								}}
								disabled={!canEdit}
							>
								{signingEnabled ? 'Enabled' : 'Disabled'}
							</Button>
						</div>
						<p class="text-xs text-muted-foreground">
							When enabled, all new traces are Ed25519-signed and hash-chained.
						</p>
					</div>

					<div class="grid gap-2">
						<Label for="verification">Chain Verification Interval (hours)</Label>
						<Input
							id="verification"
							type="number"
							min="1"
							bind:value={verificationHours}
							disabled={!canEdit}
						/>
					</div>

					{#if canEdit}
						<Button type="submit" disabled={saving}>
							{saving ? 'Saving...' : 'Save Settings'}
						</Button>
					{:else}
						<p class="text-sm text-muted-foreground">
							Only owners and admins can modify compliance settings.
						</p>
					{/if}
				</form>
			</div>
		</div>

		<!-- Public Key Card -->
		<div class="border-border overflow-hidden rounded-lg border">
			<div class="bg-muted/30 px-4 py-3 text-sm font-semibold">Signing Public Key</div>
			<div class="p-4 space-y-3">
				<p class="text-xs text-muted-foreground mb-2">
					Use this key to independently verify trace signatures.
				</p>
				<Button
					variant="outline"
					size="sm"
					onclick={async () => {
						try {
							const res = await api.get<{ algorithm: string; public_key: string }>(
								`/api/v1/orgs/${slug}/compliance/public-key`
							);
							navigator.clipboard.writeText(res.public_key);
							success = 'Public key copied to clipboard.';
						} catch {
							error = 'Failed to fetch public key';
						}
					}}
				>
					Copy Public Key
				</Button>
			</div>
		</div>
	{/if}
</div>
{:else}
	<EnterpriseUpgrade feature="compliance" />
{/if}
