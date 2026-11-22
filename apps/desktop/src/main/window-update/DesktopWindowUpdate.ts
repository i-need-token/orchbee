import { randomUUID } from 'node:crypto';

import {
	DesktopWindowUpdateRendererMessageSchema,
	type DesktopRelaunchPlace,
	type DesktopWindowUpdateMainMessage,
} from '@orchbee/desktop-client-protocol';

import type { DesktopDiagnosticRecorder } from '../diagnostics';
import type { ServerStartAnswer, UpdateRestart } from '../server-connection';
import { resolveWindowUpdateOffer } from './window-update-offer';

/**
 * The `client-server` capability that marks a Session of the first automatic-update release or
 * later, so a newer Client may join it (`crates/connection-handshake/src/registry.rs`).
 */
const COMPATIBILITY_WINDOW = 'compatibility-window';

/** How long a place request waits for its document's reply before the switch goes ahead without one. */
const PLACE_REQUEST_TIMEOUT_MILLISECONDS = 1_000;

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
	readonly recordEvent: DesktopDiagnosticRecorder;
	/**
	 * Through the bundled Client of window version `version`, restarts the Session's Server and Worker
	 * when that version asks for it or cannot join (`joins` false), keeping every terminal.
	 */
	readonly restartForVersion: (
		desktopRoot: string,
		version: string,
		joins: boolean,
	) => Promise<UpdateRestart>;
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
 * a Setup launch falls back to focusing this window.
 */
export class DesktopWindowUpdate {
	#checking = false;
	#disposed = false;
	#focusSequence = 0;
	#offer: string | null = null;
	readonly #options: DesktopWindowUpdateOptions;
	readonly #ports = new Set<ServedPort>();
	#reading: Promise<void> | null = null;
	/** The Host replacement a document was asked about, until its answer. */
	#replacing:
		| {
				readonly place: DesktopRelaunchPlace | null;
				readonly served: ServedPort;
				readonly version: string;
		  }
		| undefined;
	#rereadRequested = false;
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
		port.postMessage({ type: 'desktop-window-update-offered', version: this.#offer });
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
		for (const served of this.#ports) {
			served.port.postMessage({ type: 'desktop-window-update-offered', version: offer });
		}
	}

	/**
	 * Whether version `version` can serve this window after its start `answer`: `ready` when it joins
	 * the Session, and restarts it first when it asks for that, or it cannot join and restarted it;
	 * `host-required` when a Terminal Host must be replaced first. Otherwise the diagnostic log
	 * records why the update waits.
	 */
	async #prepare(
		root: string,
		version: string,
		answer: ServerStartAnswer | null,
	): Promise<'host-required' | 'ready' | 'waiting'> {
		const joins = answer?.negotiatedCapabilities.includes(COMPATIBILITY_WINDOW) === true;
		const restart = await this.#options
			.restartForVersion(root, version, joins)
			.catch((): UpdateRestart => 'failed');
		if (this.#disposed) return 'waiting';
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
		const root = this.#options.desktopRoot;
		if (root === null || this.#checking || version !== this.#offer) return;
		this.#checking = true;
		try {
			const answer = await this.#options.startVersionClient(root, version).catch(() => null);
			if (this.#disposed) return;
			const prepared = await this.#prepare(root, version, answer);
			if (prepared === 'ready') {
				const started = await this.#options.onJoinable(version, place).catch(() => false);
				if (started === false && served.open) {
					served.port.postMessage({ type: 'desktop-window-update-failed' });
				}
				return;
			}
			if (prepared === 'host-required' && served.open) {
				this.#replacing = { place, served, version };
				served.port.postMessage({ type: 'desktop-window-update-host-replacement' });
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
	 * with every terminal restarted; declined, or once the stop fails, the update waits.
	 */
	async #replaceHost(served: ServedPort, confirmed: boolean): Promise<void> {
		const replacing = this.#replacing;
		if (replacing?.served !== served || this.#checking) return;
		this.#replacing = undefined;
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
}
