<script lang="ts">
	import { ArrowUp } from '@lucide/svelte';
	import type { MentionItem, MentionRef } from '$lib/types';

	let {
		onSend,
		disabled,
		mentions = []
	}: {
		onSend: (text: string, mentions: MentionRef[]) => void;
		disabled: boolean;
		mentions?: MentionItem[];
	} = $props();

	let text = $state('');
	let textarea: HTMLTextAreaElement | undefined = $state();
	let showDropdown = $state(false);
	let mentionQuery = $state('');
	let selectedIndex = $state(0);
	let activeMentions = $state<MentionRef[]>([]);

	$effect(() => {
		textarea?.focus();
	});

	const filteredMentions = $derived.by(() => {
		if (!mentionQuery && !showDropdown) return [];
		const q = mentionQuery.toLowerCase();
		return mentions
			.filter((m) => {
				if (!q) return true;
				if (m.display.toLowerCase().includes(q)) return true;
				if (m.email && m.email.toLowerCase().includes(q)) return true;
				return false;
			})
			.slice(0, 8);
	});

	function getMentionTrigger(): { start: number; query: string } | null {
		if (!textarea) return null;
		const pos = textarea.selectionStart;
		const before = text.slice(0, pos);
		const match = before.match(/@([\w.\-]*)$/);
		if (!match) return null;
		return { start: pos - match[0].length, query: match[1] };
	}

	function handleInput() {
		const trigger = getMentionTrigger();
		if (trigger) {
			mentionQuery = trigger.query;
			showDropdown = true;
			selectedIndex = 0;
		} else {
			showDropdown = false;
			mentionQuery = '';
		}
	}

	function selectMention(item: MentionItem) {
		const trigger = getMentionTrigger();
		if (!trigger || !textarea) return;

		const before = text.slice(0, trigger.start);
		const after = text.slice(textarea.selectionStart);
		text = `${before}@${item.display} ${after}`;
		showDropdown = false;
		mentionQuery = '';

		activeMentions = [
			...activeMentions,
			{
				type: item.type,
				id: item.id,
				display: item.display
			}
		];

		const cursorPos = before.length + 1 + item.display.length + 1;
		requestAnimationFrame(() => {
			textarea?.setSelectionRange(cursorPos, cursorPos);
			textarea?.focus();
		});
	}

	function handleKeydown(e: KeyboardEvent) {
		if (showDropdown && filteredMentions.length > 0) {
			if (e.key === 'ArrowDown') {
				e.preventDefault();
				selectedIndex = (selectedIndex + 1) % filteredMentions.length;
				return;
			}
			if (e.key === 'ArrowUp') {
				e.preventDefault();
				selectedIndex = (selectedIndex - 1 + filteredMentions.length) % filteredMentions.length;
				return;
			}
			if (e.key === 'Enter' || e.key === 'Tab') {
				e.preventDefault();
				selectMention(filteredMentions[selectedIndex]);
				return;
			}
			if (e.key === 'Escape') {
				e.preventDefault();
				showDropdown = false;
				return;
			}
		}

		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			submit();
		}
	}

	function submit() {
		const trimmed = text.trim();
		if (!trimmed || disabled) return;
		onSend(trimmed, activeMentions);
		text = '';
		activeMentions = [];
		showDropdown = false;
	}

	function categoryColor(type: 'user' | 'repo' | 'model'): string {
		switch (type) {
			case 'user':
				return 'bg-primary/20 text-primary';
			case 'repo':
				return 'bg-emerald-500/20 text-emerald-600 dark:text-emerald-400';
			case 'model':
				return 'bg-amber-500/20 text-amber-600 dark:text-amber-400';
		}
	}

	function categoryLabel(type: 'user' | 'repo' | 'model'): string {
		switch (type) {
			case 'user':
				return 'User';
			case 'repo':
				return 'Repo';
			case 'model':
				return 'Model';
		}
	}
</script>

<div class="border-t border-border bg-background px-4 py-3">
	<div class="mx-auto max-w-3xl">
		<div class="relative">
			{#if showDropdown && filteredMentions.length > 0}
				<div class="absolute bottom-full left-0 right-0 mb-1 max-h-[200px] overflow-y-auto rounded-lg border border-border bg-popover shadow-lg">
					{#each filteredMentions as item, i}
						<button
							type="button"
							onmousedown={(e) => {
								e.preventDefault();
								selectMention(item);
							}}
							class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition-colors
								{i === selectedIndex ? 'bg-muted' : 'hover:bg-muted/50'}"
						>
							<span class="inline-flex shrink-0 items-center rounded px-1.5 py-0.5 text-[10px] font-medium {categoryColor(item.type)}">
								{categoryLabel(item.type)}
							</span>
							<span class="truncate font-medium text-foreground">{item.display}</span>
							{#if item.email}
								<span class="truncate text-xs text-muted-foreground">{item.email}</span>
							{/if}
						</button>
					{/each}
				</div>
			{/if}

			<div class="flex items-end gap-2 rounded-xl border border-border bg-muted/30 p-2 transition-colors focus-within:border-ring focus-within:ring-1 focus-within:ring-ring">
				<textarea
					bind:this={textarea}
					bind:value={text}
					onkeydown={handleKeydown}
					oninput={handleInput}
					{disabled}
					placeholder="Ask about your sessions, commits, or code... Use @ to mention users, repos, or models"
					rows={1}
					class="flex-1 resize-none bg-transparent px-2 py-1.5 text-sm text-foreground placeholder:text-muted-foreground/60 focus:outline-none disabled:opacity-50"
				></textarea>
				<button
					onclick={submit}
					disabled={disabled || !text.trim()}
					class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground transition-all hover:bg-primary/90 disabled:opacity-30 disabled:hover:bg-primary"
				>
					<ArrowUp class="h-4 w-4" />
				</button>
			</div>
		</div>
		<p class="mt-1.5 text-center text-[10px] text-muted-foreground/50">
			Searches across your session transcripts using AI
		</p>
	</div>
</div>
