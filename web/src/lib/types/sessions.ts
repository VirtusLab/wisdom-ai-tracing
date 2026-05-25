/** Session list item (from /traces/sessions endpoint) */
export interface SessionItem {
	id: string;
	session_id: string;
	repo_id: string;
	repo_name: string;
	user_id: string | null;
	user_email: string | null;
	status: string;
	model: string | null;
	tool: string | null;
	total_tool_calls: number | null;
	total_tokens: number | null;
	input_tokens: number | null;
	output_tokens: number | null;
	cache_read_tokens: number | null;
	cache_write_tokens: number | null;
	estimated_cost_usd: number | null;
	cwd: string | null;
	started_at: string | null;
	updated_at: string | null;
}

/** Session info in the detail response */
export interface SessionInfo {
	id: string;
	session_id: string;
	repo_name: string;
	user_email: string | null;
	status: string;
	model: string | null;
	tool: string | null;
	total_tool_calls: number | null;
	total_tokens: number | null;
	estimated_cost_usd: number | null;
	cwd: string | null;
	started_at: string | null;
	ended_at: string | null;
	updated_at: string | null;
}

export interface EventItem {
	id: string;
	event_index: number;
	event_type: string;
	tool_name: string | null;
	tool_input: unknown | null;
	tool_response: unknown | null;
	timestamp: string;
}

export interface FileChange {
	id: string;
	file_path: string;
	change_type: string;
	diff_text: string | null;
	content_hash: string | null;
	timestamp: string;
}

export interface TranscriptChunk {
	chunk_index: number;
	data: unknown;
}

export interface LinkedCommit {
	commit_id: string;
	commit_sha: string;
	branch: string | null;
	confidence: number | null;
}

export interface TranscriptRecordUsage {
	input_tokens: number;
	output_tokens: number;
	cache_read_tokens: number;
	cache_write_tokens: number;
	cost_usd: number;
}

export interface TranscriptRecordData {
	record_type: string;
	timestamp: string | null;
	content_types: string[];
	tool_name: string | null;
	text: string | null;
	usage: TranscriptRecordUsage | null;
	model: string | null;
}

export interface SessionCounts {
	events: number;
	file_changes: number;
	transcript_records: number;
	linked_commits: number;
}

export interface SessionMetadataResponse {
	session: SessionInfo;
	counts: SessionCounts;
}

export interface TranscriptResponse {
	transcript_chunks: TranscriptChunk[];
	transcript_records: TranscriptRecordData[];
}
