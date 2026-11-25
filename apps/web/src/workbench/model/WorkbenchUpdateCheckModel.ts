import type {
	WorkbenchClientUpdate,
	WorkbenchUpdateCheck,
	WorkbenchUpdateCheckAnswer,
	WorkbenchUpdateCheckStatus,
	WorkbenchUpdateCheckUnavailableReason,
} from '../types/workbench-client-update';

/** How often a shown check reads a queued or running run. */
const READ_INTERVAL_MILLISECONDS = 1000;
/**
 * How long an installed version may take to reach the host's offer. Main follows the window
 * pointer as it changes, and the TUI reads its store every 10 seconds; past this the status goes
 * back to Idle rather than claiming either outcome.
 */
const OFFER_WAIT_MILLISECONDS = 15_000;

const IDLE: WorkbenchUpdateCheckStatus = { kind: 'idle' };
const CHECKING: WorkbenchUpdateCheckStatus = { kind: 'checking' };
const UP_TO_DATE: WorkbenchUpdateCheckStatus = { kind: 'up-to-date' };
const FAILED: WorkbenchUpdateCheckStatus = { kind: 'failed' };
const SERVER_UNAVAILABLE: WorkbenchUpdateCheckStatus = {
	kind: 'unavailable',
	reason: 'installation',
};

interface WorkbenchUpdateCheckModelOptions {
	/** One `workbench.update-check` exchange; it must not throw, and never waits for a run. */
	readonly ask: (action: 'read' | 'start') => Promise<WorkbenchUpdateCheckAnswer>;
	/** The host's offer, which an installed version reaches. */
	readonly offer: Pick<WorkbenchClientUpdate, 'getSnapshot' | 'subscribe'>;
	/** Why the check can never run in this window, decided before any request; null when it can. */
	readonly unavailable: WorkbenchUpdateCheckUnavailableReason | null;
}

/**
 * Check for updates over the Session Server's automatic-update job, as the TUI's About runs it.
 * One exchange runs at a time. A finished result shows only for a run this window asked for, and an
 * installed version shows as the host's offer, never as Up to date.
 */
export class WorkbenchUpdateCheckModel implements WorkbenchUpdateCheck {
	#asking = false;
	#awaitingOffer: ReturnType<typeof setTimeout> | null = null;
	#disposed = false;
	readonly #listeners = new Set<() => void>();
	readonly #options: WorkbenchUpdateCheckModelOptions;
	#readTimer: ReturnType<typeof setTimeout> | null = null;
	#shown = 0;
	/** This window asked for the run being read, so its end is this window's result. */
	#started = false;
	#startQueued = false;
	#status: WorkbenchUpdateCheckStatus;
	readonly #stopOffer: () => void;

	constructor(options: WorkbenchUpdateCheckModelOptions) {
		this.#options = options;
		this.#status =
			options.unavailable === null ? IDLE : { kind: 'unavailable', reason: options.unavailable };
		this.#stopOffer = options.offer.subscribe(() => this.#offerChanged());
	}

	readonly getSnapshot = (): WorkbenchUpdateCheckStatus => this.#status;

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	readonly start = (): void => {
		if (this.#disposed || this.#status.kind === 'checking' || this.#status.kind === 'unavailable') {
			return;
		}
		this.#started = true;
		this.#set(CHECKING);
		// A read already on its way is answered for a run this press did not ask for.
		if (this.#asking) this.#startQueued = true;
		else void this.#ask('start');
	};

	readonly show = (): (() => void) => {
		this.#shown += 1;
		if (this.#shown === 1 && !this.#disposed && this.#options.unavailable === null) {
			if (this.#status.kind === 'up-to-date' || this.#status.kind === 'failed') this.#set(IDLE);
			if (this.#awaitingOffer === null && !this.#asking) void this.#ask('read');
		}
		let shown = true;
		return () => {
			if (!shown) return;
			shown = false;
			this.#shown -= 1;
			if (this.#shown === 0) this.#clearRead();
		};
	};

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#clearRead();
		this.#clearOfferWait();
		this.#stopOffer();
		this.#listeners.clear();
	}

	async #ask(action: 'read' | 'start'): Promise<void> {
		this.#asking = true;
		let answer: WorkbenchUpdateCheckAnswer;
		try {
			answer = await this.#options.ask(action);
		} catch {
			answer = 'not-answered';
		}
		this.#asking = false;
		if (this.#disposed) return;
		if (this.#startQueued) {
			this.#startQueued = false;
			void this.#ask('start');
			return;
		}
		this.#receive(answer);
	}

	#receive(answer: WorkbenchUpdateCheckAnswer): void {
		switch (answer) {
			case 'checking':
				this.#set(CHECKING);
				this.#scheduleRead();
				return;
			case 'up-to-date':
				this.#finish(UP_TO_DATE);
				return;
			case 'installed':
				if (!this.#started || this.#options.offer.getSnapshot()?.kind === 'client') {
					this.#finish(IDLE);
					return;
				}
				// Stays Checking until the host offers the installed version.
				this.#started = false;
				this.#clearOfferWait();
				this.#awaitingOffer = setTimeout(() => {
					this.#awaitingOffer = null;
					this.#set(IDLE);
				}, OFFER_WAIT_MILLISECONDS);
				return;
			// A run this window asked for that ran nothing, failed, or whose answer was lost failed.
			case 'idle':
			case 'not-answered':
			case 'run-failed':
				this.#finish(FAILED);
				return;
			case 'unavailable':
				this.#started = false;
				this.#set(SERVER_UNAVAILABLE);
				return;
		}
	}

	/** A run ended: its result is this window's only when this window asked for it. */
	#finish(result: WorkbenchUpdateCheckStatus): void {
		this.#set(this.#started ? result : IDLE);
		this.#started = false;
	}

	#offerChanged(): void {
		if (this.#awaitingOffer === null || this.#options.offer.getSnapshot()?.kind !== 'client')
			return;
		this.#clearOfferWait();
		this.#set(IDLE);
	}

	#scheduleRead(): void {
		this.#clearRead();
		if (this.#shown === 0) return;
		this.#readTimer = setTimeout(() => {
			this.#readTimer = null;
			if (!this.#disposed && !this.#asking && this.#status.kind === 'checking') {
				void this.#ask('read');
			}
		}, READ_INTERVAL_MILLISECONDS);
	}

	#clearRead(): void {
		if (this.#readTimer !== null) clearTimeout(this.#readTimer);
		this.#readTimer = null;
	}

	#clearOfferWait(): void {
		if (this.#awaitingOffer !== null) clearTimeout(this.#awaitingOffer);
		this.#awaitingOffer = null;
	}

	#set(status: WorkbenchUpdateCheckStatus): void {
		if (status === this.#status) return;
		this.#status = status;
		for (const listener of this.#listeners) {
			try {
				listener();
			} catch {
				// A listener's failure never stops the check or the other listeners.
			}
		}
	}
}
