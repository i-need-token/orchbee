import type { FileError, FileResult } from '../file-error';
import type { MemorySessionState } from './memory-session-state';

interface PendingAnswer {
	/** Computes the answer and gives it. */
	readonly give: () => void;
	/** Settles with an error; `ran` first computes the answer and drops it, as a lost reply does. */
	readonly settle: (error: FileError, ran: boolean) => void;
	readonly session: MemorySessionState;
}

function failed<Value>(code: FileError['code']): Promise<FileResult<Value>> {
	return Promise.resolve({ error: { code }, kind: 'error' });
}

/**
 * When the in-memory machine answers requests, and whether its link is up. Every answer is
 * computed when it is given, so a test that holds answers sees them reflect what changed
 * meanwhile. Immediate answers arrive in request order as microtasks; manual answers wait for
 * `flush`.
 */
export class MemoryAnswering {
	private linked = true;
	private readonly manual: boolean;
	private readonly pending: PendingAnswer[] = [];

	public constructor(mode: 'immediate' | 'manual') {
		this.manual = mode === 'manual';
	}

	public get connected(): boolean {
		return this.linked;
	}

	public get size(): number {
		return this.pending.length;
	}

	public answer<Value>(
		session: MemorySessionState,
		signal: AbortSignal,
		compute: () => FileResult<Value>,
	): Promise<FileResult<Value>> {
		if (session.closed || signal.aborted) return failed('cancelled');
		if (!this.linked) return failed('remote-disconnected');
		const pending = this.pending;
		return new Promise((resolve) => {
			let settled = false;
			const entry: PendingAnswer = { give, session, settle };
			pending.push(entry);
			signal.addEventListener('abort', cancel, { once: true });
			if (!this.manual) queueMicrotask(give);

			function finish(result: () => FileResult<Value>): void {
				if (settled) return;
				settled = true;
				const index = pending.indexOf(entry);
				if (index >= 0) pending.splice(index, 1);
				signal.removeEventListener('abort', cancel);
				resolve(result());
			}
			function give(): void {
				finish(compute);
			}
			function settle(error: FileError, ran: boolean): void {
				finish(() => {
					if (ran) compute();
					return { error, kind: 'error' };
				});
			}
			function cancel(): void {
				settle({ code: 'cancelled' }, false);
			}
		});
	}

	/** Gives every answer waiting now, in request order, and returns how many it gave. */
	public flush(): number {
		const waiting = this.pending.splice(0);
		for (const entry of waiting) entry.give();
		return waiting.length;
	}

	/** Cancels one closed session's waiting answers. */
	public cancel(session: MemorySessionState): void {
		const waiting = this.pending.filter((entry) => entry.session === session);
		for (const entry of waiting) entry.settle({ code: 'cancelled' }, false);
	}

	/**
	 * Drops the link: waiting answers and every request until `reconnect` settle
	 * `remote-disconnected`. With `delivered`, each waiting request runs first and only its reply
	 * is lost.
	 */
	public disconnect(delivered: boolean): void {
		this.linked = false;
		const waiting = this.pending.splice(0);
		for (const entry of waiting) entry.settle({ code: 'remote-disconnected' }, delivered);
	}

	public reconnect(): void {
		this.linked = true;
	}
}
