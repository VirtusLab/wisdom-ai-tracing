<script lang="ts">
	import Send from '@lucide/svelte/icons/send';

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

<div class="border-t border-border p-3">
	<div class="flex items-end gap-2">
		<textarea
			bind:this={textarea}
			bind:value={text}
			onkeydown={handleKeydown}
			{disabled}
			placeholder="Ask about your sessions, commits, or code..."
			rows={1}
			class="flex-1 resize-none rounded-md border border-border bg-background p-3 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring disabled:opacity-50"
		></textarea>
		<button
			onclick={submit}
			disabled={disabled || !text.trim()}
			class="shrink-0 rounded-md bg-primary p-3 text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
		>
			<Send class="h-4 w-4" />
		</button>
	</div>
</div>
