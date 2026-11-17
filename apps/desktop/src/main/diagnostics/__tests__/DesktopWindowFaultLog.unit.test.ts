import { describe, expect, it } from 'vitest';

import type { DesktopDiagnosticRecorder } from '../DesktopDiagnosticLog';
import { DesktopWindowFaultLog } from '../DesktopWindowFaultLog';

function createLog() {
	const records: unknown[][] = [];
	const recordEvent: DesktopDiagnosticRecorder = (...record) => void records.push(record);
	let now = 0;
	let pending: (() => void) | undefined;
	const log = new DesktopWindowFaultLog(recordEvent, {
		clearTimeout: () => {
			pending = undefined;
		},
		now: () => now,
		setTimeout: (callback) => {
			pending = callback;
			return setTimeout(() => undefined, 0);
		},
	});
	return {
		advance(milliseconds: number) {
			now += milliseconds;
		},
		fireTimer() {
			pending?.();
		},
		log,
		records,
	};
}

describe('DesktopWindowFaultLog', () => {
	it('keeps a page that keeps failing to a bounded number of records and counts the rest', () => {
		const { fireTimer, log, records } = createLog();
		for (let fault = 0; fault < 10_000; fault += 1) log.record('message-dispatch');

		const written = records.filter(([, event]) => event === 'window-fault').length;
		expect(written).toBeGreaterThan(0);
		expect(written).toBeLessThan(10_000);

		fireTimer();
		expect(records.at(-1)).toEqual([
			'warning',
			'window-faults-dropped',
			{ count: 10_000 - written },
		]);
	});

	it('records again once the period has passed', () => {
		const { advance, log, records } = createLog();
		for (let fault = 0; fault < 10_000; fault += 1) log.record('terminal-step');
		const before = records.length;

		advance(24 * 60 * 60 * 1000);
		log.record('workbench-render');

		expect(records.slice(before).at(-1)).toEqual([
			'warning',
			'window-fault',
			{ site: 'workbench-render' },
		]);
	});
});
