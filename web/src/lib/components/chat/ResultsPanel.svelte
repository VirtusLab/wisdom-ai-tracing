<script lang="ts">
	import type { ChatSessionRef, ChatCommitRef } from '$lib/types';
	import { fmtRelativeTime } from '$lib/utils/format';
	import X from '@lucide/svelte/icons/x';

	let {
		sessions,
		commits,
		slug,
		onClose
	}: {
		sessions: ChatSessionRef[];
		commits: ChatCommitRef[];
		slug: string;
		onClose: () => void;
	} = $props();
</script>

<div class="flex h-full w-80 shrink-0 flex-col border-l border-border bg-background">
	<div class="flex items-center justify-between border-b border-border px-4 py-3">
		<span class="text-sm font-semibold">References</span>
		<button onclick={onClose} class="text-muted-foreground hover:text-foreground transition-colors">
			<X class="h-4 w-4" />
		</button>
	</div>

	<div class="flex-1 overflow-y-auto p-4 space-y-6">
		{#if sessions.length > 0}
			<div>
				<h3 class="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
					Referenced Sessions
				</h3>
				<div class="space-y-2">
					{#each sessions as s}
						<a
							href="/orgs/{slug}/traces/sessions/{s.session_id}"
							target="_blank"
							rel="noopener"
							class="block rounded-md border border-border p-3 transition-colors hover:bg-muted"
						>
							<div class="flex items-center justify-between">
								<span class="font-mono text-xs">{s.session_external_id.slice(0, 12)}</span>
								<span class="text-[10px] text-muted-foreground">{fmtRelativeTime(s.started_at)}</span>
							</div>
							<div class="mt-1 text-xs text-muted-foreground">
								{s.repo_name}
								{#if s.user_email}
									<span class="mx-1">&middot;</span>{s.user_email}
								{/if}
							</div>
							{#if s.summary_snippet}
								<p class="mt-1.5 text-xs text-foreground/80 line-clamp-2">{s.summary_snippet}</p>
							{/if}
						</a>
					{/each}
				</div>
			</div>
		{/if}

		{#if commits.length > 0}
			<div>
				<h3 class="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
					Referenced Commits
				</h3>
				<div class="space-y-2">
					{#each commits as c}
						<a
							href="/orgs/{slug}/traces/sessions/{c.session_id}"
							target="_blank"
							rel="noopener"
							class="block rounded-md border border-border p-3 transition-colors hover:bg-muted"
						>
							<span class="font-mono text-xs text-primary">{c.sha.slice(0, 7)}</span>
							<p class="mt-1 text-xs text-foreground/80 line-clamp-2">{c.message}</p>
						</a>
					{/each}
				</div>
			</div>
		{/if}

		{#if sessions.length === 0 && commits.length === 0}
			<p class="text-sm text-muted-foreground text-center py-8">No references found.</p>
		{/if}
	</div>
</div>
