import { browser } from '$app/environment';
import { goto } from '$app/navigation';

const BASE_URL = import.meta.env.PUBLIC_API_URL || '';

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
		throw new Error('Unauthorized');
	}

	if (!resp.ok) {
		const body = await resp.text();
		let message = body || `HTTP ${resp.status}`;
		try {
			const parsed = JSON.parse(body);
			if (parsed.error) message = parsed.error;
		} catch {
			// not JSON, use raw body
		}
		throw new Error(message);
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
