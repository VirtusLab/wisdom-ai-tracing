import { fmtNum, fmtCost } from './format';

/**
 * Sonnet rate weights used for proportional cost apportionment across token types.
 * Approximate — model mix varies across sessions, but Sonnet rates are used as a
 * consistent reference point (same approach across all analytics views).
 */
export const TOKEN_RATE_WEIGHTS = {
	input: 3.0,
	output: 15.0,
	cacheWrite: 3.75,
	cacheRead: 0.3
} as const;

export interface TokenBreakdown {
	input: number;
	output: number;
	cacheWrite: number;
	cacheRead: number;
}

export interface SplitRow {
	label: string;
	value: string;
}

/** Build the token count rows for a SplitStatCard. */
export function tokenSplitRows(t: TokenBreakdown): SplitRow[] {
	return [
		{ label: 'Input', value: fmtNum(t.input) },
		{ label: 'Output', value: fmtNum(t.output) },
		{ label: 'Cache write', value: fmtNum(t.cacheWrite) },
		{ label: 'Cache read', value: fmtNum(t.cacheRead) }
	];
}

/** Build the cost apportionment rows for a SplitStatCard. Returns [] when total cost is zero. */
export function costSplitRows(t: TokenBreakdown, totalCost: number): SplitRow[] {
	const r = TOKEN_RATE_WEIGHTS;
	const weighted =
		t.input * r.input + t.output * r.output + t.cacheWrite * r.cacheWrite + t.cacheRead * r.cacheRead;
	if (weighted === 0) return [];
	return [
		{ label: 'Input', value: fmtCost(((t.input * r.input) / weighted) * totalCost) },
		{ label: 'Output', value: fmtCost(((t.output * r.output) / weighted) * totalCost) },
		{ label: 'Cache write', value: fmtCost(((t.cacheWrite * r.cacheWrite) / weighted) * totalCost) },
		{ label: 'Cache read', value: fmtCost(((t.cacheRead * r.cacheRead) / weighted) * totalCost) }
	];
}
