import type { RequestHandler } from './$types';

import { env } from '$env/dynamic/private';

const API_SERVER = env.API_URL || import.meta.env.PUBLIC_API_URL || 'http://localhost:3000';

const handler: RequestHandler = async ({ request, params, url }) => {
	const target = `${API_SERVER}/api/${params.path}${url.search}`;
	const headers = new Headers(request.headers);
	headers.delete('host');

	const hasBody = request.method !== 'GET' && request.method !== 'HEAD';
	const body = hasBody ? await request.text() : undefined;

	return fetch(target, {
		method: request.method,
		headers,
		body
	});
};

export const GET = handler;
export const POST = handler;
export const PUT = handler;
export const DELETE = handler;
export const OPTIONS = handler;
