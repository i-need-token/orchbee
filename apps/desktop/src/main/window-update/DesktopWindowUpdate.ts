import { randomUUID } from 'node:crypto';

import {
	DesktopWindowUpdateRendererMessageSchema,
	type DesktopRelaunchPlace,
	type DesktopSessionUpdateState,
	type DesktopWindowUpdateMainMessage,
	type DesktopWindowUpdateOffer,
} from '@orchbee/desktop-client-protocol';

import type { DesktopDiagnosticRecorder } from '../diagnostics';
import { isNewerProductVersion } from '../product-version';
import type {
	ServerStartAnswer,
	SessionMoves,
	TerminalsStaying,
	UpdateRestart,
	UpdateRestartRequest,
} from '../server-connection';
import { resolveWindowUpdateOffer } from './window-update-offer';

/**
 * The `client-server` capability that marks a Session of the first automatic-update release or
 * later, so a newer Client may join it (`crates/connection-handshake/src/registry.rs`).
 */
const COMPATIBILITY_WINDOW = 'compatibility-window';

/** How long a place request waits for its document's reply before the switch goes ahead without one. */
const PLACE_REQUEST_TIMEOUT_MILLISECONDS = 1_000;

/** How often the moves off older Terminal Hosts are read: the Worker moves one terminal per turn. */
const MOVES_READ_PERIOD_MILLISECONDS = 2_000;
/** How many reads the moves get after a connect before what is left counts as normal. */
const MOVES_READS = 90;

/** Main's end of one document's window-update port, as Electron's `MessagePortMain` offers it. */
export interface DesktopWindowUpdatePort {
	close(): void;
	on(event: 'message', listener: (event: { readonly data: unknown }) => void): void;
	postMessage(message: DesktopWindowUpdateMainMessage): void;
	start(): void;
}

/** Whether a document's window has focus, and how Main hears that the window gains it. */
export interface DesktopWindowUpdateFocus {
	isFocused(): boolean;
	/** Calls `listener` whenever the window gains focus; returns what stops those calls. */
	onFocus(listener: () => void): () => void;
}

export interface DesktopWindowUpdateOptions {
	/** `desktop\` when the window runs from one of its version folders; null keeps the offer empty. */
	readonly desktopRoot: string | null;
	/** The version a fallback start came back from, which the window never offers while it runs. */
	readonly fallbackFrom: string | null;
	/**
	 * The new version's Client joins the Session; the window now switches to that version at the place
	 * the asking document shows, returning `false` when launching it fails so this window stays.
	 * Further requests wait until it settles.
	 */
	readonly onJoinable: (
		version: string,
		place: DesktopRelaunchPlace | null,
	) => Promise<boolean | void>;
	readonly ownVersion: string;
	/** The active window version the pointer names, or null. */
	readonly readPointer: (desktopRoot: string) => Promise<string | null>;
	/**
	 * Through this window's own Client, reads the terminals that Terminal Hosts older than the
	 * Session's Worker run, and whether a failed move stopped moving them; `undefined` when the read
	 * fails.
	 */
	readonly readSessionMoves: () => Promise<SessionMoves | undefined>;
	readonly recordEvent: DesktopDiagnosticRecorder;
	/**
	 * Through the bundled Client of window version `version`, restarts the Session's Server and Worker
	 * when that version asks for it or cannot join (`joins` false), keeping every terminal.
	 */
	readonly restartForVersion: (
		desktopRoot: string,
		version: string,
		request: UpdateRestartRequest,
	) => Promise<UpdateRestart>;
	/**
	 * Through this window's own Client, restarts the Session's Server and Worker onto this window's
	 * version, keeping every terminal.
	 */
	readonly restartSession: (request: UpdateRestartRequest) => Promise<UpdateRestart>;
	/** Runs `server start --format json` through the bundled Client of window version `version`. */
	readonly startVersionClient: (desktopRoot: string, version: string) => Promise<ServerStartAnswer>;
	/**
	 * Stops the Session through the window's page, as Quit All does, and settles true once the stop
	 * has ended; a confirmed Host replacement runs it before the switch.
	 */
	readonly stopSession: () => Promise<boolean>;
	/**
	 * Calls back whenever the pointer may have changed; returns what ends the watch, or null when the
	 * folder cannot be watched now.
	 */
	readonly watchPointer: (desktopRoot: string, onChange: () => void) => (() => void) | null;
}

interface PendingPlace {
	readonly requestId: string;
	/** Ends the request with `place`, or with null, and clears it from its document. */
	readonly settle: (place: DesktopRelaunchPlace | null) => void;
}

/**
 * The Session's move onto this window's version while its Server runs the older `sessionVersion`,
 * or a Terminal Host of that version still runs terminals: running now, or waiting for the reason
 * `state` names.
 */
interface SessionMove {
	readonly sessionVersion: string;
	readonly state: DesktopSessionUpdateState;
}

/** The Host replacement a document was asked about, until its answer. */
type HostReplacement =
	| {
			/** A switch to window `version`, and whether the Session already runs it. */
			readonly kind: 'window';
			readonly place: DesktopRelaunchPlace | null;
			readonly restarted: boolean;
			readonly served: ServedPort;
			readonly version: string;
	  }
	| {
			/** A move of the Session onto this window's own version. */
			readonly kind: 'session';
			readonly served: ServedPort;
	  };

interface ServedPort {
	/** The focus sequence number of the document's latest focus: a higher number is more recent. */
	focusedAt: number;
	readonly focus: DesktopWindowUpdateFocus;
	open: boolean;
	pendingPlace: PendingPlace | undefined;
	readonly port: DesktopWindowUpdatePort;
	stopFollowingFocus: () => void;
}

/**
 * The window's Update control, as Main owns it: the newer window version the pointer names, offered
 * to every document, and a document's request, or a Setup launch's, to switch to it. A request first
 * checks that the new version's Client joins this Session, and has that Client restart the Session
 * when it asks for it or cannot join; when neither serves, a document is told the update waits, and
 * a Setup launch falls back to focusing this window. A Session whose Server runs an older version
 * than this window is moved onto this window's version by itself once a document connects; only a
 * move that cannot happen now is offered, with why it waits, until a request or a later document
 * carries it out. A Session that runs this window's version has the moves of terminals off its
 * older Terminal Hosts watched after each connect, and a failed move that stopped them with
 * terminals left there offers the move as well, since a restart's Worker moves them again.
 */
export class DesktopWindowUpdate {
	#checking = false;
	#disposed = false;
	#focusSequence = 0;
	/** The latest watch of the moves off older Hosts: an earlier one ends at its next step. */
	#movesWatch = 0;
	#offer: string | null = null;
	readonly #options: DesktopWindowUpdateOptions;
	readonly #ports = new Set<ServedPort>();
	/** The offer every served document was last sent. */
	#published: DesktopWindowUpdateOffer | null = null;
	#reading: Promise<void> | null = null;
	#replacing: HostReplacement | undefined;
	#rereadRequested = false;
	#session: SessionMove | null = null;
	#stopWatching: (() => void) | undefined;

	constructor(options: DesktopWindowUpdateOptions) {
		this.#options = options;
	}

	/** Whether this window can ever be offered a newer version: only from a release store's folder. */
	get offersUpdates(): boolean {
		return this.#options.desktopRoot !== null;
	}

	/** The newer window version this window offers, or null while it offers none. */
	get offeredVersion(): string | null {
		return this.#offer;
	}

	/**
	 * Once a document connected after the start `answer`: when the Session's Server runs a version
	 * older than this window, has this window's Client restart it onto this window's version, keeping
	 * every terminal. Every document reads that the Session updates meanwhile; the connection then
	 * ends and the window loads again once the new Server serves. A move that cannot happen now is
	 * offered with why it waits. A Session as new as this window, or one that names no version,
	 * clears that offer; a move or a switch already running leaves everything to it. A Session as
	 * new as this window then has its moves of terminals off older Terminal Hosts watched, which
	 * alone settles a move offered because they stopped.
	 */
	async moveSession(answer: ServerStartAnswer): Promise<void> {
		const sessionVersion = answer.sessionVersion;
		if (
			sessionVersion === null ||
			!isNewerProductVersion(this.#options.ownVersion, sessionVersion)
		) {
			if (sessionVersion === null || this.#session?.state !== 'moves-stopped') {
				this.#setSession(null);
			}
			if (sessionVersion !== null) void this.#watchMoves();
			return;
		}
		if (this.#checking || this.#disposed) return;
		this.#checking = true;
		try {
			this.#setSession({ sessionVersion, state: 'updating' });
			const restart = await this.#options
				.restartSession({
					joins: answer.negotiatedCapabilities.includes(COMPATIBILITY_WINDOW),
					sessionOlder: true,
				})
				.catch((): UpdateRestart => 'failed');
			if (this.#disposed) return;
			this.#settleSession(sessionVersion, restart);
		} finally {
			this.#checking = false;
		}
	}

	/**
	 * Before a refused start shows its refusal: a Session too old for this window to join is
	 * restarted onto this window's version where a restart keeps its terminals, and true once it was,
	 * so the caller starts again; false leaves the refusal to the page.
	 */
	async restartRefusedSession(answer: ServerStartAnswer): Promise<boolean> {
		const restore = answer.restore;
		if (
			restore.kind !== 'refused' ||
			restore.refusal.reason !== 'session-too-old' ||
			this.#checking ||
			this.#disposed
		) {
			return false;
		}
		this.#checking = true;
		try {
			const restart = await this.#options
				.restartSession({ joins: false, sessionOlder: true })
				.catch((): UpdateRestart => 'failed');
			const restarted =
				restart === 'restarted' ||
				(typeof restart === 'object' && restart.kind === 'terminals-staying');
			this.#options.recordEvent(
				'info',
				restarted ? 'session-update-restarted' : 'session-update-refused',
			);
			return restarted && !this.#disposed;
		} finally {
			this.#checking = false;
		}
	}

	/** Whether a switch is being checked or prepared, a Session restart included. */
	get checking(): boolean {
		return this.#checking;
	}

	/** Reads the pointer and follows its changes; a window outside a version folder offers nothing. */
	start(): void {
		this.#follow();
	}

	/**
	 * Reads the pointer from disk now (or extends a read already running) so the offer reflects the
	 * current `active.json` even when `fs.watch` has not fired yet.
	 */
	refresh(): Promise<void> {
		const root = this.#options.desktopRoot;
		if (root === null || this.#disposed) return Promise.resolve();
		this.#follow();
		return this.#startRead(root);
	}

	/**
	 * Refreshes the pointer from disk and, when it offers a newer version whose Client joins this
	 * Session, switches to that version. Answers whether the launcher started; when false, the caller
	 * keeps and focuses this window.
	 */
	async switchIfOffered(
		resolvePlace?: () => Promise<DesktopRelaunchPlace | null> | DesktopRelaunchPlace | null,
	): Promise<boolean> {
		const root = this.#options.desktopRoot;
		if (root === null || this.#disposed || this.#checking) return false;
		await this.refresh();
		const version = this.#offer;
		if (version === null || this.#disposed || this.#checking) return false;
		this.#checking = true;
		try {
			const answer = await this.#options.startVersionClient(root, version).catch(() => null);
			if (this.#disposed) return false;
			if ((await this.#prepare(root, version, answer)) !== 'ready') return false;
			const place =
				resolvePlace === undefined ? null : await Promise.resolve(resolvePlace()).catch(() => null);
			if (this.#disposed) return false;
			const started = await this.#options.onJoinable(version, place).catch(() => false);
			return started !== false;
		} finally {
			this.#checking = false;
		}
	}

	/**
	 * Serves one document's port: the current offer at once and every change, its requests, and its
	 * replies to place requests. `focus` tells whether the document's window has focus, which is where
	 * a place is asked of. A document is served once a Session start has answered, and on macOS the
	 * first start of a window with no store makes the window store then, so a pointer folder not yet
	 * followed is tried again.
	 */
	serve(port: DesktopWindowUpdatePort, focus: DesktopWindowUpdateFocus): { close(): void } {
		const served: ServedPort = {
			focus,
			focusedAt: ++this.#focusSequence,
			open: true,
			pendingPlace: undefined,
			port,
			stopFollowingFocus: () => undefined,
		};
		served.stopFollowingFocus = focus.onFocus(() => {
			served.focusedAt = ++this.#focusSequence;
		});
		this.#ports.add(served);
		port.on('message', ({ data }) => {
			const message = DesktopWindowUpdateRendererMessageSchema.safeParse(data);
			if (!served.open || !message.success) return;
			switch (message.data.type) {
				case 'desktop-window-update-refresh':
					void this.refresh();
					return;
				case 'desktop-window-update-place':
					this.#settlePlace(served, message.data);
					return;
				case 'desktop-window-update-requested':
					void this.#request(served, message.data);
					return;
				case 'desktop-window-update-host-replacement-answer':
					void this.#replaceHost(served, message.data.confirmed);
					return;
			}
		});
		port.start();
		port.postMessage({ offer: this.#published, type: 'desktop-window-update-offered' });
		this.#follow();
		return {
			close: () => {
				if (!served.open) return;
				this.#release(served);
				port.close();
			},
		};
	}

	dispose(): void {
		this.#disposed = true;
		this.#stopWatching?.();
		this.#stopWatching = undefined;
		for (const served of this.#ports) {
			this.#release(served);
			served.port.close();
		}
	}

	/**
	 * The place the window being switched shows: the focused document's, else the most recently
	 * focused served document's. Resolves null when no document is served, when the asked document
	 * does not reply within a second, closes, or replies to another request.
	 */
	askPlace(): Promise<DesktopRelaunchPlace | null> {
		const served = this.#placeTarget();
		return served === undefined ? Promise.resolve(null) : this.#requestPlace(served);
	}

	/** The served document a place is asked of: the one whose window has focus, else the latest focused. */
	#placeTarget(): ServedPort | undefined {
		let latest: ServedPort | undefined;
		for (const served of this.#ports) {
			if (served.focus.isFocused()) return served;
			if (latest === undefined || served.focusedAt > latest.focusedAt) latest = served;
		}
		return latest;
	}

	#requestPlace(served: ServedPort): Promise<DesktopRelaunchPlace | null> {
		const requestId = randomUUID();
		return new Promise((resolve) => {
			let timer: ReturnType<typeof setTimeout> | undefined;
			const settle = (place: DesktopRelaunchPlace | null): void => {
				clearTimeout(timer);
				if (served.pendingPlace?.requestId === requestId) served.pendingPlace = undefined;
				resolve(place);
			};
			served.pendingPlace = { requestId, settle };
			timer = setTimeout(() => settle(null), PLACE_REQUEST_TIMEOUT_MILLISECONDS);
			try {
				served.port.postMessage({ requestId, type: 'desktop-window-update-place-requested' });
			} catch {
				settle(null);
			}
		});
	}

	/** A reply settles the pending request; one naming another request settles it with no place. */
	#settlePlace(
		served: ServedPort,
		reply: { readonly place: DesktopRelaunchPlace | null; readonly requestId: string },
	): void {
		const pending = served.pendingPlace;
		if (pending === undefined) return;
		pending.settle(pending.requestId === reply.requestId ? reply.place : null);
	}

	/** Forgets a served document: its focus listener, its pending place request, and its place in the set. */
	#release(served: ServedPort): void {
		served.open = false;
		served.stopFollowingFocus();
		this.#ports.delete(served);
		served.pendingPlace?.settle(null);
		if (this.#replacing?.served === served) this.#replacing = undefined;
	}

	/**
	 * Whether `restart` moved the Session onto this window's version, recorded in the diagnostic log.
	 * Otherwise the move is offered with why it waits; a move nothing asks for offers nothing.
	 */
	#settleSession(sessionVersion: string, restart: UpdateRestart): boolean {
		if (
			restart === 'restarted' ||
			(typeof restart === 'object' && restart.kind === 'terminals-staying')
		) {
			this.#options.recordEvent('info', 'session-update-restarted');
			this.#setSession(null);
			return true;
		}
		if (restart === 'not-needed') {
			this.#setSession(null);
			return false;
		}
		let state: Exclude<DesktopSessionUpdateState, 'updating'>;
		if (restart === 'host-required') state = 'host-required';
		else if (restart === 'failed') state = 'other';
		else state = restart.reason;
		this.#options.recordEvent('info', 'session-update-waiting', { reason: state });
		this.#setSession({ sessionVersion, state });
		return false;
	}

	#setSession(session: SessionMove | null): void {
		this.#session = session;
		this.#publish();
	}

	/**
	 * Reads the Session's moves every two seconds, at most {@link MOVES_READS} times, while Terminal
	 * Hosts older than its Worker run terminals that no failed move stopped moving. Once one did,
	 * the move is offered as waiting on `moves-stopped`, once no move or switch runs, unless another
	 * move is offered; moves that run again or left nothing behind withdraw that offer. A read that
	 * fails changes nothing; a later connect starts the watch again.
	 */
	async #watchMoves(): Promise<void> {
		const watch = ++this.#movesWatch;
		for (let read = 1; ; read += 1) {
			const moves = await this.#options.readSessionMoves().catch(() => undefined);
			if (this.#disposed || watch !== this.#movesWatch) return;
			const offered = this.#session?.state === 'moves-stopped';
			if (moves?.stopped === true && !this.#checking) {
				if (this.#session !== null && !offered) return;
				if (!offered) {
					this.#options.recordEvent('info', 'session-update-waiting', {
						reason: 'moves-stopped',
					});
				}
				this.#setSession({ sessionVersion: moves.version, state: 'moves-stopped' });
				return;
			}
			if (moves !== undefined && moves?.stopped !== true && offered) this.#setSession(null);
			if (moves === null || read >= MOVES_READS) return;
			await new Promise((resolve) => setTimeout(resolve, MOVES_READ_PERIOD_MILLISECONDS));
			if (this.#disposed || watch !== this.#movesWatch) return;
		}
	}

	/**
	 * Watches the pointer's folder unless it already does, and reads the pointer. A folder that cannot
	 * be watched yet leaves the watch to the next call.
	 */
	#follow(): void {
		const root = this.#options.desktopRoot;
		if (root === null || this.#disposed || this.#stopWatching !== undefined) return;
		this.#stopWatching =
			this.#options.watchPointer(root, () => void this.#startRead(root)) ?? undefined;
		void this.#startRead(root);
	}

	/** One read at a time; a change seen during a read reads again once it ends. */
	#startRead(root: string): Promise<void> {
		if (this.#reading !== null) {
			this.#rereadRequested = true;
			return this.#reading;
		}
		const reading = this.#read(root).finally(() => {
			this.#reading = null;
		});
		this.#reading = reading;
		return reading;
	}

	async #read(root: string): Promise<void> {
		do {
			this.#rereadRequested = false;
			const activeVersion = await this.#options.readPointer(root).catch(() => null);
			if (this.#disposed) return;
			this.#setOffer(
				resolveWindowUpdateOffer({
					activeVersion,
					fallbackFrom: this.#options.fallbackFrom,
					ownVersion: this.#options.ownVersion,
				}),
			);
		} while (this.#rereadRequested);
	}

	#setOffer(offer: string | null): void {
		if (offer === this.#offer) return;
		this.#offer = offer;
		this.#publish();
	}

	/**
	 * Sends every served document what Update offers when that changed: a newer window, which also
	 * moves the Session, before a move of the Session onto this window's own version.
	 */
	#publish(): void {
		const session = this.#session;
		let offer: DesktopWindowUpdateOffer | null = null;
		if (this.#offer !== null) offer = { kind: 'window', version: this.#offer };
		else if (session !== null) {
			offer = {
				kind: 'session',
				sessionVersion: session.sessionVersion,
				state: session.state,
				version: this.#options.ownVersion,
			};
		}
		if (sameOffer(offer, this.#published)) return;
		this.#published = offer;
		for (const served of this.#ports) {
			served.port.postMessage({ offer, type: 'desktop-window-update-offered' });
		}
	}

	/**
	 * Whether version `version` can serve this window after its start `answer`: `ready` when it joins
	 * the Session, and restarts it first when it asks for that, or it cannot join and restarted it;
	 * `host-required` when a Terminal Host must be replaced first; terminals staying once the
	 * Session runs that version and terminals could not move off a Host it must replace. Otherwise
	 * the diagnostic log records why the update waits.
	 */
	async #prepare(
		root: string,
		version: string,
		answer: ServerStartAnswer | null,
	): Promise<'host-required' | 'ready' | 'waiting' | TerminalsStaying> {
		const joins = answer?.negotiatedCapabilities.includes(COMPATIBILITY_WINDOW) === true;
		const restart = await this.#options
			.restartForVersion(root, version, { joins, sessionOlder: false })
			.catch((): UpdateRestart => 'failed');
		if (this.#disposed) return 'waiting';
		if (typeof restart === 'object' && restart.kind === 'terminals-staying') {
			this.#options.recordEvent('info', 'window-update-host-replacement', {
				terminals: restart.terminals,
			});
			return restart;
		}
		if (restart === 'restarted') {
			this.#options.recordEvent('info', 'window-update-restarted');
			return 'ready';
		}
		if (restart === 'host-required') {
			this.#options.recordEvent('info', 'window-update-host-replacement');
			return 'host-required';
		}
		if (joins) {
			if (restart === 'not-needed') {
				this.#options.recordEvent('info', 'window-update-joinable');
				return 'ready';
			}
			this.#options.recordEvent('info', 'window-update-waiting', { reason: 'restart-failed' });
			return 'waiting';
		}
		this.#recordWaiting(answer);
		return 'waiting';
	}

	#recordWaiting(answer: ServerStartAnswer | null): void {
		if (answer === null || answer.negotiatedCapabilities.length === 0) {
			this.#options.recordEvent('info', 'window-update-waiting', {
				reason: answer?.restore.kind === 'refused' ? 'start-refused' : 'no-capabilities',
			});
			return;
		}
		this.#options.recordEvent('info', 'window-update-waiting', {
			reason: 'capability-missing',
		});
	}

	/**
	 * A request for anything but the current offer, or one made while a check or its switch runs, does
	 * nothing: the offer the page shows follows Main's, and one check answers every click made during
	 * it. Changing this (the Update action and its refusal and failure notices) needs a manual
	 * real-window self-test before merging; its steps are in `apps/desktop/README.md`, Window Update,
	 * Manual self-test.
	 */
	async #request(
		served: ServedPort,
		{ place, version }: { readonly place: DesktopRelaunchPlace | null; readonly version: string },
	): Promise<void> {
		const offer = this.#published;
		if (this.#checking || offer === null || version !== offer.version) return;
		if (offer.kind === 'session') {
			await this.#requestSessionMove(served, offer.sessionVersion);
			return;
		}
		const root = this.#options.desktopRoot;
		if (root === null) return;
		this.#checking = true;
		try {
			const answer = await this.#options.startVersionClient(root, version).catch(() => null);
			if (this.#disposed) return;
			const prepared = await this.#prepare(root, version, answer);
			if (typeof prepared === 'object' && served.open) {
				this.#replacing = { kind: 'window', place, restarted: true, served, version };
				served.port.postMessage({
					terminals: prepared.terminals,
					type: 'desktop-window-update-host-replacement',
				});
				return;
			}
			// With no document left to ask, the terminals that could not move stay where they run.
			if (prepared === 'ready' || typeof prepared === 'object') {
				const started = await this.#options.onJoinable(version, place).catch(() => false);
				if (started === false && served.open) {
					served.port.postMessage({ type: 'desktop-window-update-failed' });
				}
				return;
			}
			if (prepared === 'host-required' && served.open) {
				this.#replacing = { kind: 'window', place, restarted: false, served, version };
				served.port.postMessage({
					terminals: null,
					type: 'desktop-window-update-host-replacement',
				});
				return;
			}
			if (served.open) served.port.postMessage({ type: 'desktop-window-update-waiting' });
		} finally {
			this.#checking = false;
		}
	}

	/**
	 * The person's answer to the Host replacement the document `served` asked about. Confirmed for
	 * the current offer, it stops the Session and then switches, so the newer version starts it again
	 * with every terminal restarted. Declined once the Session runs the newer version, it switches and
	 * the terminals that could not move stay on their Host; declined before, or once the stop fails,
	 * the update waits.
	 */
	async #replaceHost(served: ServedPort, confirmed: boolean): Promise<void> {
		const replacing = this.#replacing;
		if (replacing?.served !== served || this.#checking) return;
		this.#replacing = undefined;
		if (replacing.kind === 'session') {
			// Declined, the move stays offered as it was.
			if (confirmed) await this.#replaceHostForSession(served);
			return;
		}
		if (!confirmed && replacing.restarted) {
			this.#options.recordEvent('info', 'window-update-terminals-kept');
			const started = await this.#options
				.onJoinable(replacing.version, replacing.place)
				.catch(() => false);
			if (started === false && served.open) {
				served.port.postMessage({ type: 'desktop-window-update-failed' });
			}
			return;
		}
		if (!confirmed || replacing.version !== this.#offer) {
			if (served.open) served.port.postMessage({ type: 'desktop-window-update-waiting' });
			return;
		}
		this.#checking = true;
		try {
			const stopped = await this.#options.stopSession().catch(() => false);
			if (this.#disposed) return;
			if (!stopped) {
				this.#options.recordEvent('info', 'window-update-waiting', { reason: 'stop-failed' });
				if (served.open) served.port.postMessage({ type: 'desktop-window-update-waiting' });
				return;
			}
			this.#options.recordEvent('info', 'window-update-host-replaced');
			const started = await this.#options
				.onJoinable(replacing.version, replacing.place)
				.catch(() => false);
			if (started === false && served.open) {
				served.port.postMessage({ type: 'desktop-window-update-failed' });
			}
		} finally {
			this.#checking = false;
		}
	}

	/**
	 * A document's request to move the Session onto this window's version: this window's Client
	 * restarts it, keeping every terminal, after which the connection ends and the window loads again
	 * once the new Server serves. A Terminal Host that must be replaced asks the document first;
	 * a move that cannot happen tells it the update waits, with why in the offer.
	 */
	async #requestSessionMove(served: ServedPort, sessionVersion: string): Promise<void> {
		this.#checking = true;
		try {
			this.#setSession({ sessionVersion, state: 'updating' });
			const restart = await this.#options
				.restartSession({ joins: true, sessionOlder: true })
				.catch((): UpdateRestart => 'failed');
			if (this.#disposed) return;
			if (this.#settleSession(sessionVersion, restart) || !served.open) return;
			if (restart === 'host-required') {
				this.#replacing = { kind: 'session', served };
				served.port.postMessage({
					terminals: null,
					type: 'desktop-window-update-host-replacement',
				});
				return;
			}
			served.port.postMessage({ type: 'desktop-window-update-waiting' });
		} finally {
			this.#checking = false;
		}
	}

	/**
	 * The person confirmed that every terminal restarts for this window's version: the Session stops,
	 * as Quit All stops it, and the document loads again, which starts it with this window's Client.
	 */
	async #replaceHostForSession(served: ServedPort): Promise<void> {
		this.#checking = true;
		try {
			const stopped = await this.#options.stopSession().catch(() => false);
			if (this.#disposed) return;
			if (!stopped) {
				this.#options.recordEvent('info', 'session-update-waiting', { reason: 'stop-failed' });
				if (served.open) served.port.postMessage({ type: 'desktop-window-update-waiting' });
				return;
			}
			this.#options.recordEvent('info', 'session-update-host-replaced');
			this.#setSession(null);
			if (served.open) served.port.postMessage({ type: 'desktop-window-update-reload' });
		} finally {
			this.#checking = false;
		}
	}
}

function sameOffer(
	left: DesktopWindowUpdateOffer | null,
	right: DesktopWindowUpdateOffer | null,
): boolean {
	if (left === null || right === null) return left === right;
	if (left.kind === 'window' || right.kind === 'window') {
		return left.kind === right.kind && left.version === right.version;
	}
	return (
		left.version === right.version &&
		left.sessionVersion === right.sessionVersion &&
		left.state === right.state
	);
}
