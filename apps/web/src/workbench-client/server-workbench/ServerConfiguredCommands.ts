import {
	WORKBENCH_PROTOCOL_VERSION,
	parseWorkbenchRequest,
	type WorkbenchResponse,
} from '@orchbee/workbench-client-protocol';

import {
	ServerStreamRefusedError,
	decodeWorkbenchResponse,
	encodeWorkbenchRequest,
	type ServerConnection,
	type ServerSubscription,
} from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';

type ConfiguredCommandOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.configured-command.result' }
>['outcome'];

/** One command the Session's configuration installs, as the Server lists it. */
type ServerConfiguredCommand = Extract<
	ConfiguredCommandOutcome,
	{ readonly kind: 'catalog' }
>['entries'][number];

/**
 * The Panel a command starts from: the terminal it shows, and the Server's record of the view that
 * showed it, which the Server checks again before any effect.
 */
export interface ServerConfiguredCommandSource {
	readonly clientId: string;
	readonly panelId: string;
	readonly presentationId: string;
	readonly tabId: string;
	readonly terminal: { readonly terminalId: string; readonly workerId: string };
	readonly terminalContextId: string;
	readonly viewRevision: string;
}

/**
 * How an invocation ended for this window: `unsent` when it never left, `unknown` when it left and
 * no answer came back, so its effect may or may not have happened.
 */
export type ServerConfiguredCommandReply =
	| { readonly kind: 'answered'; readonly outcome: ConfiguredCommandOutcome }
	| { readonly kind: 'unknown' }
	| { readonly kind: 'unsent' };

interface ServerConfiguredCommandsOptions {
	readonly connection: Pick<ServerConnection, 'exchange' | 'subscribe'>;
	readonly sessionId: string;
}

const LIST_DEADLINE_MILLISECONDS = 5_000;
/** As the TUI bounds an invocation, which covers a Worker confirming that a shell command started. */
const INVOKE_DEADLINE_MILLISECONDS = 10_000;

/**
 * The commands the Session's configuration installs, and their invocation. The catalog is read again
 * whenever the configuration's key profile changes, since a command's identity binds that profile, so
 * a definition that changed never runs under an identity read before it.
 */
export class ServerConfiguredCommands {
	readonly #connection: ServerConfiguredCommandsOptions['connection'];
	readonly #sessionId: string;
	#disposed = false;
	#entries: readonly ServerConfiguredCommand[] = [];
	#keysProfile: string | null = null;
	readonly #listeners = new Set<() => void>();
	/** Only the latest read may replace the catalog. */
	#listing = 0;
	#observation: ServerSubscription | null = null;

	constructor({ connection, sessionId }: ServerConfiguredCommandsOptions) {
		this.#connection = connection;
		this.#sessionId = sessionId;
		this.#observe();
	}

	/** Holds its identity until the catalog changes. */
	getSnapshot = (): readonly ServerConfiguredCommand[] => this.#entries;

	subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	/** Reads the catalog again, as after a command the Server no longer knows. */
	refresh(): void {
		void this.#list();
	}

	/**
	 * Runs a command once; nothing here ever repeats it. The window reports neither a frame of its
	 * terminal nor an attachment of its own, which the Server reads only to size a new Pane.
	 */
	async invoke(
		commandId: string,
		expectedRevision: string,
		source: ServerConfiguredCommandSource | null,
	): Promise<ServerConfiguredCommandReply> {
		const requestId = createWorkbenchRequestId();
		const request = this.#request(requestId, {
			invocation: {
				commandId,
				context: {
					...(source === null ? {} : { focusedPanelId: source.panelId, tabId: source.tabId }),
					invocationSource: 'gui',
				},
				expectedRevision,
				selection: null,
				source:
					source === null
						? null
						: {
								clientId: source.clientId,
								presentation: {
									member: {
										attachmentGeneration: '0',
										binding: source.terminal,
										incarnation: 0,
										presentationId: source.presentationId,
										viewport: { reason: 'frame-unavailable', state: 'unavailable' },
									},
									panelId: source.panelId,
									tabId: source.tabId,
									terminalContextId: source.terminalContextId,
								},
								viewRevision: source.viewRevision,
							},
			},
			kind: 'invoke',
		});
		if (request === null || this.#disposed) return { kind: 'unsent' };
		let record: Uint8Array;
		try {
			record = await this.#connection.exchange(request, {
				timeoutMilliseconds: INVOKE_DEADLINE_MILLISECONDS,
			});
		} catch (error) {
			return error instanceof ServerStreamRefusedError ? { kind: 'unsent' } : { kind: 'unknown' };
		}
		const outcome = this.#outcome(record, requestId);
		return outcome === null ? { kind: 'unknown' } : { kind: 'answered', outcome };
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#observation?.close();
		this.#observation = null;
		this.#listeners.clear();
	}

	#observe(): void {
		const requestId = createWorkbenchRequestId();
		let request: Uint8Array;
		try {
			const parsed = parseWorkbenchRequest({
				action: 'current',
				kind: 'workbench.configuration',
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId,
				sessionId: this.#sessionId,
			});
			request = encodeWorkbenchRequest(parsed);
		} catch {
			return;
		}
		const observed = (record: Uint8Array): void => {
			let response: WorkbenchResponse;
			try {
				response = decodeWorkbenchResponse(record);
			} catch {
				return;
			}
			if (
				response.kind !== 'workbench.configuration.result' ||
				response.requestId !== requestId ||
				response.outcome.kind === 'failed' ||
				response.outcome.snapshot.keysProfile === this.#keysProfile
			) {
				return;
			}
			this.#keysProfile = response.outcome.snapshot.keysProfile;
			void this.#list();
		};
		try {
			const observation = this.#connection.subscribe('state', request, {
				onClosed: () => {
					// The catalog stays as last read; a command it no longer holds refreshes it.
					if (this.#observation === observation) this.#observation = null;
				},
				onFrame: (_kind, record) => {
					if (this.#observation === observation) observed(record);
				},
				// Every frame is a complete snapshot, so a gap loses nothing.
				onGap: () => undefined,
			});
			this.#observation = observation;
		} catch {
			// The connection has ended, which its Workbench session reports.
		}
	}

	async #list(): Promise<void> {
		if (this.#disposed) return;
		const listing = ++this.#listing;
		const requestId = createWorkbenchRequestId();
		const request = this.#request(requestId, { kind: 'list' });
		if (request === null) return;
		let record: Uint8Array;
		try {
			record = await this.#connection.exchange(request, {
				timeoutMilliseconds: LIST_DEADLINE_MILLISECONDS,
			});
		} catch {
			// Unanswered, the catalog stays as last read.
			return;
		}
		if (this.#disposed || listing !== this.#listing) return;
		const outcome = this.#outcome(record, requestId);
		if (outcome === null) return;
		// A refusal says this window can run nothing, so it offers nothing.
		const entries = outcome.kind === 'catalog' ? outcome.entries : [];
		if (JSON.stringify(entries) === JSON.stringify(this.#entries)) return;
		this.#entries = entries;
		for (const listener of new Set(this.#listeners)) {
			if (!this.#listeners.has(listener)) continue;
			try {
				listener();
			} catch {
				/* Isolate catalog consumers. */
			}
		}
	}

	#request(requestId: string, action: unknown): Uint8Array | null {
		try {
			const request = parseWorkbenchRequest({
				action,
				kind: 'workbench.configured-command',
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId,
				sessionId: this.#sessionId,
			});
			return request.kind === 'workbench.configured-command'
				? encodeWorkbenchRequest(request)
				: null;
		} catch {
			// The window described the invocation outside what the protocol admits.
			return null;
		}
	}

	#outcome(record: Uint8Array, requestId: string): ConfiguredCommandOutcome | null {
		try {
			const response = decodeWorkbenchResponse(record);
			return response.kind === 'workbench.configured-command.result' &&
				response.requestId === requestId &&
				response.sessionId === this.#sessionId
				? response.outcome
				: null;
		} catch {
			return null;
		}
	}
}
