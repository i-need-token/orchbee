import type { DesktopWindowFaultSite } from '@orchbee/desktop-client-protocol';

import type { DesktopDiagnosticRecorder } from './DesktopDiagnosticLog';

/** A window records at most this many faults in any one period; the rest are counted. */
const RECORDS_PER_PERIOD = 20;
const PERIOD_MILLISECONDS = 60_000;

interface DesktopWindowFaultLogClock {
	clearTimeout(timer: ReturnType<typeof setTimeout>): void;
	now(): number;
	setTimeout(callback: () => void, milliseconds: number): ReturnType<typeof setTimeout>;
}

const systemClock: DesktopWindowFaultLogClock = {
	clearTimeout: (timer) => clearTimeout(timer),
	now: () => Date.now(),
	setTimeout: (callback, milliseconds) => setTimeout(callback, milliseconds),
};

/**
 * Writes one window's faults, which its page reports by site, to the daily diagnostic log. A page
 * that keeps failing cannot fill the log: past the bound in a period its faults are dropped, and
 * once the period ends one record says how many.
 */
export class DesktopWindowFaultLog {
	readonly #clock: DesktopWindowFaultLogClock;
	#dropped = 0;
	#flushTimer: ReturnType<typeof setTimeout> | undefined;
	#periodStart = Number.NEGATIVE_INFINITY;
	#recorded = 0;
	readonly #recordEvent: DesktopDiagnosticRecorder;

	constructor(
		recordEvent: DesktopDiagnosticRecorder,
		clock: DesktopWindowFaultLogClock = systemClock,
	) {
		this.#recordEvent = recordEvent;
		this.#clock = clock;
	}

	record(site: DesktopWindowFaultSite): void {
		const now = this.#clock.now();
		if (now - this.#periodStart >= PERIOD_MILLISECONDS) {
			this.#flush();
			this.#periodStart = now;
			this.#recorded = 0;
		}
		if (this.#recorded < RECORDS_PER_PERIOD) {
			this.#recorded += 1;
			this.#recordEvent('warning', 'window-fault', { site });
			return;
		}
		this.#dropped += 1;
		this.#flushTimer ??= this.#clock.setTimeout(
			() => this.#flush(),
			this.#periodStart + PERIOD_MILLISECONDS - now,
		);
	}

	/** Writes what was dropped so far; the window this log belongs to is gone. */
	dispose(): void {
		this.#flush();
	}

	#flush(): void {
		if (this.#flushTimer !== undefined) this.#clock.clearTimeout(this.#flushTimer);
		this.#flushTimer = undefined;
		if (this.#dropped === 0) return;
		this.#recordEvent('warning', 'window-faults-dropped', { count: this.#dropped });
		this.#dropped = 0;
	}
}
