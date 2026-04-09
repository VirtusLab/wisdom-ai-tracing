export interface Conversation {
	id: string;
	org_id: string;
	user_id: string;
	title: string | null;
	created_at: string;
	updated_at: string;
}

export interface ChatMessage {
	id: string;
	conversation_id: string;
	role: 'user' | 'assistant';
	content: string;
	referenced_sessions: string[] | null;
	referenced_commits: string[] | null;
	filters_applied: ExtractedFilters | null;
	created_at: string;
}

export interface ExtractedFilters {
	query: string;
	user: string | null;
	repo: string | null;
	time_from: string | null;
	time_to: string | null;
	model: string | null;
}

export interface ChatSessionRef {
	session_id: string;
	session_external_id: string;
	repo_name: string;
	user_email: string | null;
	started_at: string | null;
	summary_snippet: string;
}

export interface ChatCommitRef {
	sha: string;
	message: string;
	session_id: string;
}

export interface SendMessageResponse {
	content: string;
	filters: ExtractedFilters;
	referenced_sessions: ChatSessionRef[];
	referenced_commits: ChatCommitRef[];
}

export interface ConversationWithMessages {
	conversation: Conversation;
	messages: ChatMessage[];
}

export interface MentionItem {
	type: 'user' | 'repo' | 'model';
	id?: string;
	display: string;
	email?: string;
}

export interface MentionRef {
	type: 'user' | 'repo' | 'model';
	id?: string;
	display: string;
}

export interface MentionsResponse {
	users: Array<{ id: string; display: string; email: string }>;
	repos: Array<{ id: string; display: string }>;
	models: Array<{ display: string }>;
}
