import { writable } from 'svelte/store';
import { browser } from '$app/environment';
import { api } from '$lib/api';

export interface FeatureFlags {
	edition: string;
	compliance: boolean;
	audit_trail: boolean;
	sso: boolean;
	story_generation: boolean;
	advanced_analytics: boolean;
	multi_org: boolean;
	encryption_at_rest: boolean;
	full_policy_engine: boolean;
	advanced_redaction: boolean;
	chat_search: boolean;
	initialized: boolean;
	loaded: boolean;
}

const communityDefaults: FeatureFlags = {
	edition: 'community',
	compliance: false,
	audit_trail: false,
	sso: false,
	story_generation: false,
	advanced_analytics: false,
	multi_org: false,
	encryption_at_rest: false,
	full_policy_engine: false,
	advanced_redaction: false,
	chat_search: false,
	initialized: false,
	loaded: false
};

function createFeaturesStore() {
	const { subscribe, set } = writable<FeatureFlags>(communityDefaults);

	return {
		subscribe,
		async init() {
			if (!browser) return;
			try {
				const flags = await api.get<FeatureFlags>('/api/v1/features');
				set({ ...flags, loaded: true });
			} catch {
				set({ ...communityDefaults, loaded: true });
			}
		}
	};
}

export const features = createFeaturesStore();
