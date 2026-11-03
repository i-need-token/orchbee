import {
	WORKBENCH_PROTOCOL_VERSION,
	parseWorkbenchRequest,
	type WorkbenchRequest,
	type WorkbenchResponse,
} from '@orchbee/workbench-client-protocol';

import {
	decodeWorkbenchResponse,
	encodeWorkbenchRequest,
	type ServerConnection,
} from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';

type ClientViewRequest = Extract<WorkbenchRequest, { readonly kind: 'workbench.client-view' }>;
type Synchronize = Extract<ClientViewRequest['action'], { readonly kind: 'synchronize' }>;
type ClientViewOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.client-view.result' }
>['outcome'];

/** A control the Server delivered to this window, which answers it exactly once. */
export type ServerClientViewControl = NonNullable<
	Extract<ClientViewOutcome, { readonly kind: 'synchronized' }>['control']
>;

/** How this window answers a control it was delivered. */
export type ServerClientViewOutcome = NonNullable<Synchronize['completion']>['outcome'];

/** What this window reports of itself, before the protocol checks it. */
export interface ServerClientViewSample {
	readonly phase: Synchronize['view']['phase'];
	readonly selection: {
		readonly panelCount: number;
		readonly panelId: string | null;
		readonly tabId: string | null;
		readonly terminalContextId: string;
		readonly zoomed: boolean;
	} | null;
	readonly workbenchRevision: string;
}

/**
 * The Server's record of a view the window showed, which an invocation names as its source: `stale`
 * when the window moved before reporting it, `unavailable` when the window no longer publishes.
 */
export type ServerClientViewBinding =
	| { readonly clientId: string; readonly kind: 'bound'; readonly viewRevision: string }
	| { readonly kind: 'stale' | 'unavailable' };

interface ServerClientViewOptions {
	readonly connection: Pick<ServerConnection, 'closeLane' | 'exchange' | 'openLane' | 'subscribe'>;
	/** Runs a control delivered while the window still shows the view it was admitted against. */
	readonly onControl: (control: ServerClientViewControl) => void;
	/** Reads what the window shows now, or null while that is not settled enough to report. */
	readonly sample: () => ServerClientViewSample | null;
	readonly sessionId: string;
}

interface Answer {
	readonly operationId: string;
	readonly outcome: ServerClientViewOutcome;
	/** The view the outcome was observed in, which travels with it; null sends the current one. */
	readonly view: ServerClientViewSample | null;
}

interface Bind {
	/** The last Synchronize sent before the bind, which may still carry an older view. */
	readonly after: bigint;
	readonly key: string;
	readonly settle: (binding: ServerClientViewBinding) => void;
}

/** What one Synchronize carried, so its answer can settle the binds waiting for it. */
interface Sent {
	readonly heartbeat: bigint;
	readonly key: string;
	/** An answer's own view rather than what the window shows now. */
	readonly pinned: boolean;
	readonly requestId: string;
	readonly view: ServerClientViewSample;
}

/** As the TUI bounds each Synchronize. */
const SYNCHRONIZE_DEADLINE_MILLISECONDS = 2_000;
/** One Synchronize already in flight, then the one that reports the view being bound. */
const BIND_DEADLINE_MILLISECONDS = 2 * SYNCHRONIZE_DEADLINE_MILLISECONDS;
const MAXIMUM_SEQUENCE = 2n ** 64n - 1n;

/**
 * Publishes this window's view to the Server, as the TUI's control lane publishes its own, so that the
 * CLI can find this window and route a navigation to it. One lane owns the view, and closing it retires
 * the view and settles a control the window never answered. A Synchronize is always its own exchange,
 * since it can carry a control back. The window synchronizes when its view changes, when it answers a
 * control, and when a commit invalidation says a control may wait for it, never on a timer. A failed or
 * refused Synchronize stops publishing, and nothing is ever sent again.
 */
export class ServerClientView {
	readonly #connection: ServerClientViewOptions['connection'];
	readonly #laneId: bigint;
	readonly #onControl: ServerClientViewOptions['onControl'];
	readonly #sample: ServerClientViewOptions['sample'];
	readonly #sessionId: string;
	/** The view the Server last acknowledged, and the record it holds of it. */
	#acknowledged: {
		readonly clientId: string;
		readonly key: string;
		readonly viewRevision: string;
	} | null = null;
	#answer: Answer | null = null;
	readonly #binds = new Set<Bind>();
	/** The control being run; the Server delivers no other until it is answered. */
	#delivered: string | null = null;
	#heartbeat = 0n;
	#inFlight = false;
	/** A commit may have queued a control, so the next Synchronize goes even with the same view. */
	#invalidated = false;
	#published: string | null = null;
	#stopped = false;
	#viewRevision = 0n;
	/** Nothing synchronizes before the invalidation stream opens, so no queued control goes unannounced. */
	#watching = false;

	private constructor(options: ServerClientViewOptions, laneId: bigint) {
		this.#connection = options.connection;
		this.#laneId = laneId;
		this.#onControl = options.onControl;
		this.#sample = options.sample;
		this.#sessionId = options.sessionId;
	}

	/** Starts publishing, or returns null when the connection holds no lane for it. */
	static open(options: ServerClientViewOptions): ServerClientView | null {
		let laneId: bigint;
		try {
			laneId = options.connection.openLane();
		} catch {
			return null;
		}
		const view = new ServerClientView(options, laneId);
		view.#watch();
		return view;
	}

	/**
	 * Binds a view the window shows to the Server's record of it, as the TUI binds a configured command:
	 * the view is reported first if it is new, and the first Synchronize carrying what the window shows
	 * afterwards decides. A view the window left before reporting it is `stale`.
	 */
	bind(sample: ServerClientViewSample): Promise<ServerClientViewBinding> {
		if (this.#stopped) return Promise.resolve({ kind: 'unavailable' });
		const key = sampleKey(sample);
		this.#synchronize();
		const acknowledged = this.#acknowledged;
		if (!this.#inFlight && acknowledged?.key === key) {
			return Promise.resolve({
				clientId: acknowledged.clientId,
				kind: 'bound',
				viewRevision: acknowledged.viewRevision,
			});
		}
		if (this.#stopped) return Promise.resolve({ kind: 'unavailable' });
		return new Promise((resolve) => {
			const bind: Bind = {
				after: this.#heartbeat,
				key,
				settle: (binding) => {
					clearTimeout(deadline);
					this.#binds.delete(bind);
					resolve(binding);
				},
			};
			const deadline = setTimeout(
				() => bind.settle({ kind: 'unavailable' }),
				BIND_DEADLINE_MILLISECONDS,
			);
			this.#binds.add(bind);
		});
	}

	/** Reports the window's view again if it changed. */
	changed(): void {
		this.#synchronize();
	}

	/** Answers the control being run; the next Synchronize carries the answer and the view it names. */
	complete(
		operationId: string,
		outcome: ServerClientViewOutcome,
		view: ServerClientViewSample | null = null,
	): void {
		if (this.#stopped || this.#delivered !== operationId) return;
		this.#delivered = null;
		this.#answer = { operationId, outcome, view };
		this.#synchronize();
	}

	dispose(): void {
		this.#stop();
	}

	#watch(): void {
		const invalidated = (): void => {
			this.#watching = true;
			this.#invalidated = true;
			this.#synchronize();
		};
		try {
			this.#connection.subscribe(
				'invalidations',
				encodeWorkbenchRequest({
					kind: 'workbench.snapshot.read',
					protocolVersion: WORKBENCH_PROTOCOL_VERSION,
					requestId: createWorkbenchRequestId(),
				}),
				{ onClosed: () => this.#stop(), onFrame: invalidated, onGap: invalidated },
				this.#laneId,
			);
		} catch {
			this.#stop();
		}
	}

	#synchronize(): void {
		if (this.#stopped || this.#inFlight || !this.#watching) return;
		const answer = this.#answer;
		const view = answer?.view ?? this.#sample();
		if (view === null) return;
		const key = sampleKey(view);
		if (answer === null && !this.#invalidated && key === this.#published) return;
		const heartbeat = this.#heartbeat + 1n;
		const viewRevision = key === this.#published ? this.#viewRevision : this.#viewRevision + 1n;
		const request =
			heartbeat > MAXIMUM_SEQUENCE || viewRevision > MAXIMUM_SEQUENCE
				? null
				: this.#request(heartbeat, viewRevision, view, answer);
		if (request === null) {
			this.#stop();
			return;
		}
		this.#answer = null;
		this.#heartbeat = heartbeat;
		this.#inFlight = true;
		this.#invalidated = false;
		this.#published = key;
		this.#viewRevision = viewRevision;
		const sent: Sent = {
			heartbeat,
			key,
			pinned: answer !== null && answer.view !== null,
			requestId: request.requestId,
			view,
		};
		void this.#connection
			.exchange(encodeWorkbenchRequest(request), {
				laneId: this.#laneId,
				timeoutMilliseconds: SYNCHRONIZE_DEADLINE_MILLISECONDS,
			})
			.then(
				(record) => this.#synchronized(record, sent),
				// A lost answer may have delivered a control, so nothing is sent again.
				() => this.#stop(),
			);
	}

	#request(
		heartbeat: bigint,
		viewRevision: bigint,
		view: ServerClientViewSample,
		answer: Answer | null,
	): ClientViewRequest | null {
		try {
			const request = parseWorkbenchRequest({
				action: {
					completion:
						answer === null ? null : { operationId: answer.operationId, outcome: answer.outcome },
					heartbeat: `${heartbeat}`,
					kind: 'synchronize',
					view,
					viewRevision: `${viewRevision}`,
				},
				kind: 'workbench.client-view',
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId: createWorkbenchRequestId(),
				sessionId: this.#sessionId,
			});
			return request.kind === 'workbench.client-view' ? request : null;
		} catch {
			// The window described itself outside what the protocol admits.
			return null;
		}
	}

	#synchronized(record: Uint8Array, sent: Sent): void {
		this.#inFlight = false;
		if (this.#stopped) return;
		let outcome: ClientViewOutcome | null = null;
		try {
			const response = decodeWorkbenchResponse(record);
			if (
				response.kind === 'workbench.client-view.result' &&
				response.requestId === sent.requestId &&
				response.sessionId === this.#sessionId
			) {
				outcome = response.outcome;
			}
		} catch {
			// A response of another cohort stops publishing below.
		}
		// A conflict, a denial or a Server holding all the views it can stops publishing, as in the TUI.
		if (outcome?.kind !== 'synchronized') {
			this.#stop();
			return;
		}
		// Only a current record can name an invocation's source, as the Server checks again.
		const { availability, clientId, viewRevision } = outcome.client;
		const current = availability === 'current';
		this.#acknowledged = current ? { clientId, key: sent.key, viewRevision } : null;
		for (const bind of this.#binds) {
			if (current && bind.key === sent.key) bind.settle({ clientId, kind: 'bound', viewRevision });
			else if (sent.heartbeat > bind.after && !sent.pinned) bind.settle({ kind: 'stale' });
		}
		const { control } = outcome;
		if (control !== null) {
			if (this.#delivered !== null) {
				this.#stop();
				return;
			}
			this.#delivered = control.operationId;
			const shown = this.#sample();
			if (shown === null || sampleKey(shown) !== sent.key) {
				// As in the TUI, the window moved after reporting the view the control was admitted against.
				this.complete(control.operationId, { kind: 'rejected', reason: 'stale-view' });
			} else {
				this.#onControl(control);
			}
		}
		this.#synchronize();
	}

	#stop(): void {
		if (this.#stopped) return;
		this.#stopped = true;
		this.#acknowledged = null;
		this.#answer = null;
		this.#delivered = null;
		for (const bind of this.#binds) bind.settle({ kind: 'unavailable' });
		// Closing the lane retires this window's view and settles a control it never answered.
		this.#connection.closeLane(this.#laneId);
	}
}

function sampleKey({ phase, selection, workbenchRevision }: ServerClientViewSample): string {
	return JSON.stringify([
		workbenchRevision,
		phase,
		selection === null
			? null
			: [
					selection.terminalContextId,
					selection.tabId,
					selection.panelId,
					selection.zoomed,
					selection.panelCount,
				],
	]);
}
