import { randomBytes } from 'node:crypto';

import type { DesktopSessionStopAnswer } from '@orchbee/desktop-client-protocol';

import type { ServerStopStatus } from '../server-connection';
import type { DesktopQuitAllFailure } from './desktop-background-lifecycle';

/** The page asks with a 10-second reply budget of its own; this bounds a page that never answers. */
const ANSWER_MILLISECONDS = 15_000;
/** The Server spends at most 60 seconds on one attempt, and then writes its receipt. */
const COMPLETION_MILLISECONDS = 75_000;
const STATUS_INTERVAL_MILLISECONDS = 500;

export type DesktopSessionStopResult =
	| { readonly kind: 'failed'; readonly reason: DesktopQuitAllFailure; readonly stopId: string }
	| { readonly kind: 'stopped' };

export interface DesktopSessionStopOptions {
	/** Reads the stop's progress with `server stop-status`. */
	readonly readStatus: (stopId: string) => Promise<ServerStopStatus>;
	/** Asks the window's page to stop the Session; false when no page can be asked. */
	readonly request: (stopId: string) => boolean;
}

/**
 * Quit All's Session stop. Main chooses the stop's identity and the page asks the Server for it
 * through its own connection, once; the stop has completed only when `server stop-status` proves
 * it, as `server stop` waits. A stop that cannot have been recorded fails at once.
 */
export class DesktopSessionStop {
	#awaiting:
		| { readonly resolve: (answer: DesktopSessionStopAnswer) => void; readonly stopId: string }
		| undefined;
	readonly #options: DesktopSessionStopOptions;

	constructor(options: DesktopSessionStopOptions) {
		this.#options = options;
	}

	/** The page's answer to the stop it was asked for; any other is dropped. */
	answered(stopId: string, answer: DesktopSessionStopAnswer): void {
		const awaiting = this.#awaiting;
		if (awaiting?.stopId !== stopId) return;
		this.#awaiting = undefined;
		awaiting.resolve(answer);
	}

	async stop(): Promise<DesktopSessionStopResult> {
		const stopId = createStopId();
		const answer = await this.#ask(stopId);
		switch (answer) {
			case 'busy':
			case 'not-sent':
			case 'refused':
			case 'unmanaged':
				return { kind: 'failed', reason: answer, stopId };
			case 'recorded':
			case 'unknown':
				return this.#awaitCompletion(stopId);
		}
	}

	#ask(stopId: string): Promise<DesktopSessionStopAnswer> {
		return new Promise((resolve) => {
			const timer = setTimeout(() => this.answered(stopId, 'unknown'), ANSWER_MILLISECONDS);
			this.#awaiting = {
				resolve: (answer) => {
					clearTimeout(timer);
					resolve(answer);
				},
				stopId,
			};
			if (!this.#options.request(stopId)) this.answered(stopId, 'not-sent');
		});
	}

	async #awaitCompletion(stopId: string): Promise<DesktopSessionStopResult> {
		const deadline = Date.now() + COMPLETION_MILLISECONDS;
		for (;;) {
			const status = await this.#options.readStatus(stopId);
			switch (status) {
				case 'stopped':
					return { kind: 'stopped' };
				case 'failed':
					return { kind: 'failed', reason: 'stop-failed', stopId };
				case 'busy':
				case 'unmanaged':
					return { kind: 'failed', reason: status, stopId };
				case 'undecided':
					break;
			}
			if (Date.now() + STATUS_INTERVAL_MILLISECONDS > deadline) {
				return { kind: 'failed', reason: 'unconfirmed', stopId };
			}
			await new Promise((resolve) => setTimeout(resolve, STATUS_INTERVAL_MILLISECONDS));
		}
	}
}

/** A nonzero 128-bit identity in lowercase hexadecimal, as `server stop --request-id` takes. */
function createStopId(): string {
	for (;;) {
		const id = randomBytes(16).toString('hex');
		if (/[^0]/u.test(id)) return id;
	}
}
