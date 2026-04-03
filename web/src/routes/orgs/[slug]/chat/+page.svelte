<script lang="ts">
	import { page } from '$app/stores';
	import { api } from '$lib/api';
	import { features } from '$lib/stores/features';
	import type {
		Conversation,
		ChatMessage,
		SendMessageResponse,
		ChatSessionRef,
		ChatCommitRef,
		ExtractedFilters
	} from '$lib/types';
	import ChatSidebar from '$lib/components/chat/ChatSidebar.svelte';
	import ChatMessages from '$lib/components/chat/ChatMessages.svelte';
	import ChatInput from '$lib/components/chat/ChatInput.svelte';
	import ResultsPanel from '$lib/components/chat/ResultsPanel.svelte';
	import FilterPills from '$lib/components/chat/FilterPills.svelte';
	import EnterpriseUpgrade from '$lib/components/enterprise-upgrade.svelte';

	const slug = $derived($page.params.slug ?? '');

	let conversations = $state<Conversation[]>([]);
	let activeConversationId = $state<string | null>(null);
	let messages = $state<ChatMessage[]>([]);
	let loadingConversations = $state(true);
	let loadingMessages = $state(false);
	let sending = $state(false);
	let error = $state('');

	let referencedSessions = $state<ChatSessionRef[]>([]);
	let referencedCommits = $state<ChatCommitRef[]>([]);
	let lastFilters = $state<ExtractedFilters | null>(null);
	let showResults = $state(false);

	$effect(() => {
		if (slug && $features.chat_search) {
			loadConversations();
		}
	});

	async function loadConversations() {
		loadingConversations = true;
		error = '';
		try {
			conversations = await api.get<Conversation[]>(
				`/api/v1/orgs/${slug}/chat/conversations`
			);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load conversations';
		} finally {
			loadingConversations = false;
		}
	}

	async function loadMessages(conversationId: string) {
		loadingMessages = true;
		error = '';
		try {
			const data = await api.get<{ conversation: Conversation; messages: ChatMessage[] }>(
				`/api/v1/orgs/${slug}/chat/conversations/${conversationId}`
			);
			messages = data.messages;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to load messages';
		} finally {
			loadingMessages = false;
		}
	}

	async function handleSelectConversation(id: string) {
		activeConversationId = id;
		referencedSessions = [];
		referencedCommits = [];
		lastFilters = null;
		showResults = false;
		await loadMessages(id);
	}

	async function handleCreateConversation() {
		error = '';
		try {
			const conv = await api.post<Conversation>(
				`/api/v1/orgs/${slug}/chat/conversations`
			);
			conversations = [conv, ...conversations];
			activeConversationId = conv.id;
			messages = [];
			referencedSessions = [];
			referencedCommits = [];
			lastFilters = null;
			showResults = false;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to create conversation';
		}
	}

	async function handleDeleteConversation(id: string) {
		error = '';
		try {
			await api.delete(`/api/v1/orgs/${slug}/chat/conversations/${id}`);
			conversations = conversations.filter((c) => c.id !== id);
			if (activeConversationId === id) {
				activeConversationId = null;
				messages = [];
				referencedSessions = [];
				referencedCommits = [];
				lastFilters = null;
				showResults = false;
			}
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to delete conversation';
		}
	}

	async function handleRenameConversation(id: string, title: string) {
		error = '';
		try {
			await api.patch(`/api/v1/orgs/${slug}/chat/conversations/${id}`, { title });
			conversations = conversations.map((c) =>
				c.id === id ? { ...c, title } : c
			);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to rename conversation';
		}
	}

	async function handleSendMessage(content: string) {
		if (!activeConversationId) {
			// Auto-create conversation
			await handleCreateConversation();
			if (!activeConversationId) return;
		}

		// Optimistically add user message
		const tempUserMsg: ChatMessage = {
			id: `temp-${Date.now()}`,
			conversation_id: activeConversationId,
			role: 'user',
			content,
			referenced_sessions: null,
			referenced_commits: null,
			filters_applied: null,
			created_at: new Date().toISOString()
		};
		messages = [...messages, tempUserMsg];
		sending = true;
		error = '';

		try {
			const resp = await api.post<SendMessageResponse>(
				`/api/v1/orgs/${slug}/chat/conversations/${activeConversationId}/messages`,
				{ content }
			);

			// Reload messages to get proper IDs
			await loadMessages(activeConversationId);

			// Update references
			referencedSessions = resp.referenced_sessions;
			referencedCommits = resp.referenced_commits;
			lastFilters = resp.filters;
			showResults = resp.referenced_sessions.length > 0 || resp.referenced_commits.length > 0;

			// Update conversation title if it was the first message
			const conv = conversations.find((c) => c.id === activeConversationId);
			if (conv && !conv.title) {
				await loadConversations();
			}
		} catch (err) {
			error = err instanceof Error ? err.message : 'Failed to send message';
			// Remove optimistic user message on error
			messages = messages.filter((m) => m.id !== tempUserMsg.id);
		} finally {
			sending = false;
		}
	}
</script>

<svelte:head>
	<title>Chat - TraceVault</title>
</svelte:head>

{#if !$features.chat_search}
	<div class="p-6">
		<EnterpriseUpgrade feature="chat_search" title="Chat Search" />
	</div>
{:else}
	<div class="flex h-[calc(100vh-4rem)]">
		<!-- Sidebar -->
		<ChatSidebar
			{conversations}
			{activeConversationId}
			onSelect={handleSelectConversation}
			onCreate={handleCreateConversation}
			onDelete={handleDeleteConversation}
			onRename={handleRenameConversation}
		/>

		<!-- Main chat area -->
		<div class="flex flex-1 flex-col min-w-0">
			{#if error}
				<div class="border-b border-border bg-destructive/10 px-4 py-2 text-sm text-destructive">
					{error}
				</div>
			{/if}

			{#if lastFilters}
				<FilterPills filters={lastFilters} />
			{/if}

			<ChatMessages
				{messages}
				loading={loadingMessages}
				{sending}
				{slug}
			/>

			<ChatInput
				onSend={handleSendMessage}
				disabled={sending}
			/>
		</div>

		<!-- Results panel -->
		{#if showResults}
			<ResultsPanel
				sessions={referencedSessions}
				commits={referencedCommits}
				{slug}
				onClose={() => (showResults = false)}
			/>
		{/if}
	</div>
{/if}
