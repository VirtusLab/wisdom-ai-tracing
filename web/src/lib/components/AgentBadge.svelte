<script lang="ts">
	let { tool }: { tool: string | null } = $props();

	const agents: Record<string, { label: string; color: string; bg: string }> = {
		'claude-code': {
			label: 'Claude Code',
			color: 'text-orange-700 dark:text-orange-400',
			bg: 'bg-orange-500/10 border-orange-500/20'
		},
		codex: {
			label: 'Codex',
			color: 'text-emerald-700 dark:text-emerald-400',
			bg: 'bg-emerald-500/10 border-emerald-500/20'
		},
		// Future agents — backend adapters not yet implemented
		gemini: {
			label: 'Gemini',
			color: 'text-sky-700 dark:text-sky-400',
			bg: 'bg-sky-500/10 border-sky-500/20'
		},
		cursor: {
			label: 'Cursor',
			color: 'text-violet-700 dark:text-violet-400',
			bg: 'bg-violet-500/10 border-violet-500/20'
		}
	};

	const agent = $derived(
		tool ? (agents[tool] ?? { label: tool, color: 'text-gray-600', bg: 'bg-gray-500/10 border-gray-500/20' }) : null
	);
</script>

{#if agent}
	<span
		class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium {agent.bg} {agent.color}"
	>
		{#if tool === 'claude-code'}
			<!-- Anthropic spark -->
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" class="shrink-0">
				<path d="M16.5 3L14 10.5L21 13L14 15.5L16.5 23L12 16.5L7.5 23L10 15.5L3 13L10 10.5L7.5 3L12 9.5L16.5 3Z" fill="currentColor" opacity="0.9"/>
			</svg>
		{:else if tool === 'codex'}
			<!-- OpenAI hexagon -->
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" class="shrink-0">
				<path d="M12 2L21.5 7.5V16.5L12 22L2.5 16.5V7.5L12 2Z" stroke="currentColor" stroke-width="2" fill="none"/>
				<circle cx="12" cy="12" r="3.5" fill="currentColor"/>
			</svg>
		{:else if tool === 'gemini'}
			<!-- Gemini star -->
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" class="shrink-0">
				<path d="M12 2C12 2 14 8 16 10C18 12 24 12 24 12C24 12 18 12 16 14C14 16 12 22 12 22C12 22 10 16 8 14C6 12 0 12 0 12C0 12 6 12 8 10C10 8 12 2 12 2Z" fill="currentColor" opacity="0.85"/>
			</svg>
		{:else if tool === 'cursor'}
			<!-- Cursor arrow -->
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" class="shrink-0">
				<path d="M5 3L19 12L12 13L15 21L12 22L9 14L5 17V3Z" fill="currentColor" opacity="0.85"/>
			</svg>
		{:else}
			<!-- Generic bot -->
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="shrink-0">
				<rect x="3" y="8" width="18" height="12" rx="2"/>
				<circle cx="9" cy="14" r="1.5" fill="currentColor"/>
				<circle cx="15" cy="14" r="1.5" fill="currentColor"/>
				<path d="M12 2v4"/>
			</svg>
		{/if}
		{agent.label}
	</span>
{/if}
