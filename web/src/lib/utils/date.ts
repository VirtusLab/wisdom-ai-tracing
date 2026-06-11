import { env } from '$env/dynamic/public';

export const locale = env.PUBLIC_DATE_LOCALE || 'en-US';

const dateFormatter = new Intl.DateTimeFormat(locale, {
	year: 'numeric',
	month: '2-digit',
	day: '2-digit'
});

const dateTimeFormatter = new Intl.DateTimeFormat(locale, {
	year: 'numeric',
	month: '2-digit',
	day: '2-digit',
	hour: '2-digit',
	minute: '2-digit'
});

const timeFormatter = new Intl.DateTimeFormat(locale, {
	hour: '2-digit',
	minute: '2-digit'
});

export function formatDate(iso: string | null): string {
	if (!iso) return '-';
	return dateFormatter.format(new Date(iso));
}

export function formatDateTime(iso: string | null): string {
	if (!iso) return '-';
	return dateTimeFormatter.format(new Date(iso));
}

/** Time-of-day only (hour:minute), in the configured locale. For timestamps
 * within a transcript/chat where the date is already implied by context. */
export function formatTime(iso: string | null): string {
	if (!iso) return '-';
	return timeFormatter.format(new Date(iso));
}
