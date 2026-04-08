import type { RequestHandler } from './$types';

import { env } from '$env/dynamic/private';

const API_SERVER = env.API_URL || import.meta.env.PUBLIC_API_URL || 'http://localhost:3000';

export const GET: RequestHandler = async () => {
	return fetch(`${API_SERVER}/health`);
};
