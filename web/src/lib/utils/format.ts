export function fmtNum(n: number | null | undefined): string {
	if (n == null) return '-';
	if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
	if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
	return String(n);
}

export function fmtCost(usd: number | null | undefined): string {
	if (usd == null) return '-';
	if (usd > 0 && usd < 0.01) return '<$0.01';
	return `$${usd.toFixed(2)}`;
}

export function fmtDuration(ms: number | null | undefined): string {
	if (ms == null) return '-';
	const totalSeconds = Math.floor(ms / 1000);
	const hours = Math.floor(totalSeconds / 3600);
	const minutes = Math.floor((totalSeconds % 3600) / 60);
	const seconds = totalSeconds % 60;
	if (hours >= 1) return `${hours}h ${minutes}m`;
	if (minutes >= 1) return `${minutes}m ${seconds}s`;
	return `${seconds}s`;
}

export function fmtPercent(v: number | null | undefined): string {
	if (v == null) return '-';
	return `${v.toFixed(1)}%`;
}

export function fmtTokens(n: number | null | undefined): string {
	return fmtNum(n);
}

export function fmtRelativeTime(iso: string | null | undefined): string {
	if (!iso) return '-';
	const diff = Date.now() - new Date(iso).getTime();
	const minutes = Math.floor(diff / 60000);
	const hours = Math.floor(minutes / 60);
	const days = Math.floor(hours / 24);
	if (days > 0) return `${days}d ago`;
	if (hours > 0) return `${hours}h ago`;
	if (minutes > 0) return `${minutes}m ago`;
	return 'just now';
}
