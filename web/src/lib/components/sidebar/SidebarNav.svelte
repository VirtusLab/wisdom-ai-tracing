<script lang="ts">
	import { page } from '$app/stores';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import {
		LayoutDashboard,
		FolderGit2,
		GitCommitHorizontal,
		BarChart3,
		ShieldCheck,
		Settings,
		MessageCircle
	} from '@lucide/svelte';
	import { features } from '$lib/stores/features';

	let { expanded, slug }: { expanded: boolean; slug: string } = $props();

	const navItems = $derived([
		{ href: `/orgs/${slug}/dashboard`, label: 'Dashboard', icon: LayoutDashboard },
		{ href: `/orgs/${slug}/repos`, label: 'Repos', icon: FolderGit2 },
		{ href: `/orgs/${slug}/traces`, label: 'Traces', icon: GitCommitHorizontal },
		...($features.chat_search
			? [{ href: `/orgs/${slug}/chat`, label: 'Chat', icon: MessageCircle }]
			: []),
		{ href: `/orgs/${slug}/analytics`, label: 'Analytics', icon: BarChart3 },
		{ href: `/orgs/${slug}/compliance`, label: 'Compliance', icon: ShieldCheck },
		{ href: `/orgs/${slug}/settings`, label: 'Settings', icon: Settings }
	]);

	const analyticsSubItems = $derived([
		{ href: `/orgs/${slug}/analytics`, label: 'Overview' },
		{ href: `/orgs/${slug}/analytics/tokens`, label: 'Tokens' },
		{ href: `/orgs/${slug}/analytics/models`, label: 'Models' },
		{ href: `/orgs/${slug}/analytics/authors`, label: 'Authors' },
		{ href: `/orgs/${slug}/analytics/attribution`, label: 'Attribution' },
		{ href: `/orgs/${slug}/analytics/sessions`, label: 'Sessions' },
		{ href: `/orgs/${slug}/analytics/cost`, label: 'Cost' },
		{ href: `/orgs/${slug}/analytics/software`, label: 'Software' },
		{ href: `/orgs/${slug}/analytics/ai-tools`, label: 'AI Tools' }
	]);

	const complianceSubItems = $derived([
		{ href: `/orgs/${slug}/compliance`, label: 'Dashboard' },
		{ href: `/orgs/${slug}/compliance/audit-log`, label: 'Audit Log' },
		{ href: `/orgs/${slug}/compliance/settings`, label: 'Settings' }
	]);

	const tracesSubItems = $derived([
		{ href: `/orgs/${slug}/traces/sessions`, label: 'Sessions' },
		{ href: `/orgs/${slug}/traces/commits`, label: 'Commits' },
		{ href: `/orgs/${slug}/traces/timeline`, label: 'Timeline' },
		{ href: `/orgs/${slug}/traces/attribution`, label: 'Attribution' },
		{ href: `/orgs/${slug}/traces/branches`, label: 'Branches' }
	]);

	const settingsSubItems = $derived([
		{ href: `/orgs/${slug}/settings`, label: 'Organizations' },
		{ href: `/orgs/${slug}/settings/pricing`, label: 'Pricing' },
		{ href: `/orgs/${slug}/settings/llm`, label: 'LLM' },
		...($features.chat_search
			? [{ href: `/orgs/${slug}/settings/chat`, label: 'Chat' }]
			: [])
	]);

	function isActive(href: string): boolean {
		return $page.url.pathname === href || $page.url.pathname.startsWith(href + '/');
	}

	function isExactActive(href: string): boolean {
		return $page.url.pathname === href;
	}
</script>

<nav class="flex-1 py-3 px-2 space-y-1">
	{#each navItems as item}
		{@const active = isActive(item.href)}
		{#if expanded}
			<a
				href={item.href}
				class="flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors
					{active ? 'bg-primary text-primary-foreground' : 'text-sidebar-foreground hover:bg-sidebar-accent'}"
			>
				<item.icon class="h-5 w-5 shrink-0" />
				<span>{item.label}</span>
			</a>

			{#if item.href === `/orgs/${slug}/traces` && active}
				<div class="ml-8 space-y-0.5 mt-0.5">
					{#each tracesSubItems as sub}
						<a
							href={sub.href}
							class="block rounded-md px-3 py-1.5 text-xs font-medium transition-colors
								{isExactActive(sub.href) ? 'text-primary font-semibold' : 'text-muted-foreground hover:text-foreground'}"
						>
							{sub.label}
						</a>
					{/each}
				</div>
			{/if}

			{#if item.href === `/orgs/${slug}/analytics` && active}
				<div class="ml-8 space-y-0.5 mt-0.5">
					{#each analyticsSubItems as sub}
						<a
							href={sub.href}
							class="block rounded-md px-3 py-1.5 text-xs font-medium transition-colors
								{isExactActive(sub.href) ? 'text-primary font-semibold' : 'text-muted-foreground hover:text-foreground'}"
						>
							{sub.label}
						</a>
					{/each}
				</div>
			{/if}

			{#if item.href === `/orgs/${slug}/compliance` && active}
				<div class="ml-8 space-y-0.5 mt-0.5">
					{#each complianceSubItems as sub}
						<a
							href={sub.href}
							class="block rounded-md px-3 py-1.5 text-xs font-medium transition-colors
								{isExactActive(sub.href) ? 'text-primary font-semibold' : 'text-muted-foreground hover:text-foreground'}"
						>
							{sub.label}
						</a>
					{/each}
				</div>
			{/if}

			{#if item.href === `/orgs/${slug}/settings` && active}
				<div class="ml-8 space-y-0.5 mt-0.5">
					{#each settingsSubItems as sub}
						<a
							href={sub.href}
							class="block rounded-md px-3 py-1.5 text-xs font-medium transition-colors
								{isExactActive(sub.href) ? 'text-primary font-semibold' : 'text-muted-foreground hover:text-foreground'}"
						>
							{sub.label}
						</a>
					{/each}
				</div>
			{/if}
		{:else}
			<Tooltip.Root>
				<Tooltip.Trigger>
					<a
						href={item.href}
						class="flex h-10 w-10 items-center justify-center rounded-md transition-colors
							{active ? 'bg-primary text-primary-foreground' : 'text-sidebar-foreground hover:bg-sidebar-accent'}"
					>
						<item.icon class="h-5 w-5" />
					</a>
				</Tooltip.Trigger>
				<Tooltip.Content side="right">
					{item.label}
				</Tooltip.Content>
			</Tooltip.Root>
		{/if}
	{/each}
</nav>
