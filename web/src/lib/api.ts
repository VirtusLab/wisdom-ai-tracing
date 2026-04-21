import { browser } from '$app/environment';
import { goto } from '$app/navigation';

const BASE_URL = import.meta.env.PUBLIC_API_URL || '';

export class ApiError extends Error {
	status: number;
	code?: string;
	constructor(message: string, status: number, code?: string) {
		super(message);
		this.name = 'ApiError';
		this.status = status;
		this.code = code;
	}
}

async function request<T>(
	path: string,
	options: RequestInit = {}
): Promise<T> {
	const token = browser ? localStorage.getItem('tracevault_token') : null;

	const headers: Record<string, string> = {
		'Content-Type': 'application/json',
		...((options.headers as Record<string, string>) || {})
	};

	if (token) {
		headers['Authorization'] = `Bearer ${token}`;
	}

	const resp = await fetch(`${BASE_URL}${path}`, {
		...options,
		headers
	});

	if (resp.status === 401 && browser) {
		localStorage.removeItem('tracevault_token');
		goto('/auth/login');
		throw new ApiError('Unauthorized', 401);
	}

	if (!resp.ok) {
		const body = await resp.text();
		let message = body || `HTTP ${resp.status}`;
		let code: string | undefined;
		try {
			const parsed = JSON.parse(body);
			if (parsed.error) message = parsed.error;
			if (typeof parsed.code === 'string') code = parsed.code;
		} catch {
			// not JSON, use raw body
		}
		throw new ApiError(message, resp.status, code);
	}

	if (resp.status === 204 || resp.headers.get('content-length') === '0') {
		return undefined as T;
	}

	return resp.json();
}

export const api = {
	get: <T>(path: string) => request<T>(path),
	post: <T>(path: string, body?: unknown) =>
		request<T>(path, { method: 'POST', body: body ? JSON.stringify(body) : undefined }),
	put: <T>(path: string, body?: unknown) =>
		request<T>(path, { method: 'PUT', body: body ? JSON.stringify(body) : undefined }),
	patch: <T>(path: string, body?: unknown) =>
		request<T>(path, { method: 'PATCH', body: body ? JSON.stringify(body) : undefined }),
	delete: <T>(path: string) => request<T>(path, { method: 'DELETE' })
};
