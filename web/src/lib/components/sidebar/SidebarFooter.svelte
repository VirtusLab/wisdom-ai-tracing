<script lang="ts">
	import { Button } from '$lib/components/ui/button/index.js';
	import { LogOut, Cable } from '@lucide/svelte';

	let {
		expanded,
		userEmail,
		onLogout
	}: {
		expanded: boolean;
		userEmail: string | null;
		onLogout: () => void;
	} = $props();
</script>

{#if expanded}
	<div class="border-t p-2">
		{#if userEmail}
			<p class="text-xs text-muted-foreground truncate px-3 py-1">{userEmail}</p>
		{/if}
		<Button
			variant="ghost"
			size="sm"
			class="w-full justify-start gap-2"
			href="/me/proxy"
		>
			<Cable class="h-4 w-4" />
			LLM Proxy
		</Button>
		<Button variant="ghost" size="sm" class="w-full justify-start gap-2" onclick={onLogout}>
			<LogOut class="h-4 w-4" />
			Log out
		</Button>
	</div>
{:else}
	<div class="border-t p-2 flex flex-col items-center gap-1">
		<Button
			variant="ghost"
			size="sm"
			class="h-9 w-9 p-0"
			href="/me/proxy"
			aria-label="LLM Proxy"
		>
			<Cable class="h-4 w-4" />
		</Button>
		<Button
			variant="ghost"
			size="sm"
			class="h-9 w-9 p-0"
			onclick={onLogout}
			aria-label="Log out"
		>
			<LogOut class="h-4 w-4" />
		</Button>
	</div>
{/if}
