<script lang="ts">
	import { ArrowUp } from '@lucide/svelte';

	let {
		onSend,
		disabled
	}: {
		onSend: (text: string) => void;
		disabled: boolean;
	} = $props();

	let text = $state('');
	let textarea: HTMLTextAreaElement | undefined = $state();

	$effect(() => {
		textarea?.focus();
	});

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			submit();
		}
	}

	function submit() {
		const trimmed = text.trim();
		if (!trimmed || disabled) return;
		onSend(trimmed);
		text = '';
	}
</script>

<div class="border-t border-border bg-background px-4 py-3">
	<div class="mx-auto max-w-3xl">
		<div class="flex items-end gap-2 rounded-xl border border-border bg-muted/30 p-2 transition-colors focus-within:border-ring focus-within:ring-1 focus-within:ring-ring">
			<textarea
				bind:this={textarea}
				bind:value={text}
				onkeydown={handleKeydown}
				{disabled}
				placeholder="Ask about your sessions, commits, or code..."
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
		<p class="mt-1.5 text-center text-[10px] text-muted-foreground/50">
			Searches across your session transcripts using AI
		</p>
	</div>
</div>
