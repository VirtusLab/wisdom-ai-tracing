import { describe, it, expect } from 'vitest';
import { fmtNum, fmtCost, fmtDuration, fmtPercent, fmtTokens, fmtRelativeTime } from './format';

describe('fmtNum', () => {
	it('returns dash for null', () => expect(fmtNum(null)).toBe('-'));
	it('returns dash for undefined', () => expect(fmtNum(undefined)).toBe('-'));
	it('formats millions', () => expect(fmtNum(1_500_000)).toBe('1.5M'));
	it('formats thousands', () => expect(fmtNum(2_500)).toBe('2.5k'));
	it('formats small numbers', () => expect(fmtNum(42)).toBe('42'));
	it('formats zero', () => expect(fmtNum(0)).toBe('0'));
});

describe('fmtCost', () => {
	it('returns dash for null', () => expect(fmtCost(null)).toBe('-'));
	it('returns dash for undefined', () => expect(fmtCost(undefined)).toBe('-'));
	it('formats to 2 decimal places', () => expect(fmtCost(1.5)).toBe('$1.50'));
	it('formats zero', () => expect(fmtCost(0)).toBe('$0.00'));
	it('formats small amounts', () => expect(fmtCost(0.003)).toBe('<$0.01'));
});

describe('fmtDuration', () => {
	it('returns dash for null', () => expect(fmtDuration(null)).toBe('-'));
	it('returns dash for undefined', () => expect(fmtDuration(undefined)).toBe('-'));
	it('formats seconds', () => expect(fmtDuration(30_000)).toBe('30s'));
	it('formats minutes and seconds', () => expect(fmtDuration(150_000)).toBe('2m 30s'));
	it('formats hours and minutes', () => expect(fmtDuration(3_720_000)).toBe('1h 2m'));
	it('formats zero', () => expect(fmtDuration(0)).toBe('0s'));
});

describe('fmtPercent', () => {
	it('returns dash for null', () => expect(fmtPercent(null)).toBe('-'));
	it('returns dash for undefined', () => expect(fmtPercent(undefined)).toBe('-'));
	it('formats with 1 decimal', () => expect(fmtPercent(42.567)).toBe('42.6%'));
	it('formats zero', () => expect(fmtPercent(0)).toBe('0.0%'));
});

describe('fmtTokens', () => {
	it('delegates to fmtNum', () => expect(fmtTokens(2_500)).toBe('2.5k'));
	it('returns dash for null', () => expect(fmtTokens(null)).toBe('-'));
});

describe('fmtRelativeTime', () => {
	it('returns dash for null', () => expect(fmtRelativeTime(null)).toBe('-'));
	it('returns dash for undefined', () => expect(fmtRelativeTime(undefined)).toBe('-'));
	it('returns "just now" for recent time', () => {
		expect(fmtRelativeTime(new Date().toISOString())).toBe('just now');
	});
	it('returns minutes ago', () => {
		const fiveMinAgo = new Date(Date.now() - 5 * 60_000).toISOString();
		expect(fmtRelativeTime(fiveMinAgo)).toBe('5m ago');
	});
	it('returns hours ago', () => {
		const twoHoursAgo = new Date(Date.now() - 2 * 60 * 60_000).toISOString();
		expect(fmtRelativeTime(twoHoursAgo)).toBe('2h ago');
	});
	it('returns days ago', () => {
		const threeDaysAgo = new Date(Date.now() - 3 * 24 * 60 * 60_000).toISOString();
		expect(fmtRelativeTime(threeDaysAgo)).toBe('3d ago');
	});
});
