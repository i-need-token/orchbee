import {
	WORKBENCH_PROTOCOL_VERSION,
	parseWorkbenchRequest,
	type WorkbenchResponse,
} from '@orchbee/workbench-client-protocol';

import {
	type ServerConnection,
	type ServerSubscription,
	decodeWorkbenchResponse,
	encodeWorkbenchRequest,
} from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';

type WindowTitleOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.window-title.result' }
>['outcome'];
type WindowTitleSnapshot = NonNullable<
	Extract<WindowTitleOutcome, { readonly kind: 'listening' }>['snapshot']
>;

/** What a Session's title template names as `{workspace}`, `{tab}`, `{panel}` and `{terminal_title}`. */
interface ServerWindowTitleContext {
	readonly pane: string;
	readonly tab: string;
	readonly terminalTitle: string;
	readonly workspace: string;
}

interface ListeningStream {
	readonly context: ServerWindowTitleContext;
	readonly focused: boolean;
	readonly requestId: string;
	readonly subscription: ServerSubscription;
}

/** What the TUI shows when the Server clears a title and no template renders one. */
const FALLBACK_TITLE = 'Orchbee';

/** The protocol's bound on each context value, counted in Unicode scalars. */
const CONTEXT_SCALARS = 200;

/** As the TUI bounds them: no control characters, then at most 200 scalars, none of them unpaired. */
function boundContextText(value: string): string {
	return [...value]
		.filter((scalar) => !/^[\p{Cc}\p{Cs}]$/u.test(scalar))
		.slice(0, CONTEXT_SCALARS)
		.join('');
}

function sameContext(left: ServerWindowTitleContext, right: ServerWindowTitleContext): boolean {
	return (
		left.pane === right.pane &&
		left.tab === right.tab &&
		left.terminalTitle === right.terminalTitle &&
		left.workspace === right.workspace
	);
}

/**
 * Names the window as the Session's title policy says, as the TUI names its outer terminal: a title
 * set from the CLI, else the configured template rendered with this window's display context. The
 * Server sends a title only to its foreground Client, the one that attached or gained focus last, so
 * a window behind another Client keeps the title it showed.
 */
export class ServerWindowTitle {
	readonly #apply: (title: string) => void;
	readonly #connection: Pick<ServerConnection, 'subscribe'>;
	readonly #sessionId: string;
	#disposed = false;
	#focused = false;
	/** The snapshot shown last; forgetting it shows the next one even when it is the same. */
	#shown: WindowTitleSnapshot | null = null;
	#stream: ListeningStream | null = null;

	constructor(
		connection: Pick<ServerConnection, 'subscribe'>,
		sessionId: string,
		apply: (title: string) => void,
	) {
		this.#apply = apply;
		this.#connection = connection;
		this.#sessionId = sessionId;
	}

	/** Listens with this focus and context, replacing a stream that listened with others. */
	update(focused: boolean, context: ServerWindowTitleContext): void {
		if (this.#disposed) return;
		// Gaining focus brings this window forward again, and it shows the current title anew.
		if (focused && !this.#focused) this.#shown = null;
		this.#focused = focused;
		const bounded: ServerWindowTitleContext = {
			pane: boundContextText(context.pane),
			tab: boundContextText(context.tab),
			terminalTitle: boundContextText(context.terminalTitle),
			workspace: boundContextText(context.workspace),
		};
		const previous = this.#stream;
		if (previous?.focused === focused && sameContext(previous.context, bounded)) return;
		const requestId = createWorkbenchRequestId();
		const request = this.#listenRequest(focused, bounded, requestId);
		if (request === null) return;
		const current = (): boolean => this.#stream?.requestId === requestId;
		let subscription: ServerSubscription;
		try {
			subscription = this.#connection.subscribe('state', request, {
				// A stream that ended stays the one listening, as in the TUI: only a new focus or context
				// listens again.
				onClosed: () => {
					if (current()) this.#shown = null;
				},
				onFrame: (_kind, record) => {
					if (current()) this.#observed(record, requestId);
				},
				onGap: () => {
					if (current()) this.#shown = null;
				},
			});
		} catch {
			// The connection has ended, which its Workbench session reports.
			return;
		}
		// Opening before closing keeps this connection among the Server's listeners, so it neither
		// forgets this window nor brings it forward as a newly attached one.
		this.#stream = { context: bounded, focused, requestId, subscription };
		previous?.subscription.close();
	}

	dispose(): void {
		this.#disposed = true;
		this.#stream?.subscription.close();
		this.#stream = null;
	}

	#observed(record: Uint8Array, requestId: string): void {
		let response: WorkbenchResponse;
		try {
			response = decodeWorkbenchResponse(record);
		} catch {
			return;
		}
		if (
			response.kind !== 'workbench.window-title.result' ||
			response.requestId !== requestId ||
			response.sessionId !== this.#sessionId ||
			response.outcome.kind !== 'listening'
		) {
			return;
		}
		const { snapshot } = response.outcome;
		// Behind another Client nothing arrives to show, and the title shown stays.
		if (snapshot === null) {
			this.#shown = null;
			return;
		}
		if (snapshot.revision === this.#shown?.revision && snapshot.title === this.#shown.title) return;
		this.#shown = snapshot;
		this.#apply(snapshot.title ?? FALLBACK_TITLE);
	}

	#listenRequest(
		focused: boolean,
		context: ServerWindowTitleContext,
		requestId: string,
	): Uint8Array | null {
		try {
			const request = parseWorkbenchRequest({
				action: { context, focused, kind: 'listen' },
				kind: 'workbench.window-title',
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId,
				sessionId: this.#sessionId,
			});
			return request.kind === 'workbench.window-title' ? encodeWorkbenchRequest(request) : null;
		} catch {
			return null;
		}
	}
}
