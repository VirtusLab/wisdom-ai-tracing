<script lang="ts">
	import type { Component } from 'svelte';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';

	interface Row {
		label: string;
		value: string;
	}

	interface Props {
		label: string;
		value: string;
		icon: Component;
		color?: string;
		secondary?: string;
		tooltip?: string;
		rows: Row[];
	}

	let { label, value, icon: Icon, color = '#3b82f6', secondary, tooltip, rows }: Props = $props();
</script>

<div class="bg-background rounded-lg border border-border p-4">
	<div class="flex items-center gap-3">
		<div
			class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg"
			style="background: color-mix(in oklch, {color} 15%, transparent)"
		>
			<Icon class="h-4 w-4" style="color: {color}" />
		</div>
		<div class="min-w-0 flex-1">
			<div class="text-muted-foreground text-[10px] font-semibold uppercase tracking-wider">
				{label}
				{#if tooltip}
					<Tooltip.Root>
						<Tooltip.Trigger>
							<span class="ml-1 cursor-help" style="color: #4f6ef7">?</span>
						</Tooltip.Trigger>
						<Tooltip.Portal>
							<Tooltip.Content class="max-w-xs text-xs font-normal normal-case tracking-normal">
								{tooltip}
							</Tooltip.Content>
						</Tooltip.Portal>
					</Tooltip.Root>
				{/if}
			</div>
			<div class="flex items-start gap-3">
				<div class="shrink-0">
					<div class="text-xl font-bold leading-tight">{value}</div>
					{#if secondary}
						<div class="text-muted-foreground mt-0.5 text-[11px]">{secondary}</div>
					{/if}
				</div>
				{#if rows.length > 0}
					<div class="border-border ml-auto border-l pl-3">
						{#each rows as row (row.label)}
							<div class="flex items-baseline justify-between gap-2">
								<span class="text-muted-foreground text-[10px]">{row.label}</span>
								<span class="text-[11px] font-medium tabular-nums">{row.value}</span>
							</div>
						{/each}
					</div>
				{/if}
			</div>
		</div>
	</div>
</div>
