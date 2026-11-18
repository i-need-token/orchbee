import {
	DesktopWindowUpdateRendererMessageSchema,
	type DesktopRelaunchPlace,
	type DesktopWindowUpdateMainMessage,
} from '@orchbee/desktop-client-protocol';

import type { DesktopDiagnosticRecorder } from '../diagnostics';
import type { ServerStartAnswer } from '../server-connection';
import { resolveWindowUpdateOffer } from './window-update-offer';

/**
 * The `client-server` capability that marks a Session of the first automatic-update release or
 * later, so a newer Client may join it (`crates/connection-handshake/src/registry.rs`).
 */
const COMPATIBILITY_WINDOW = 'compatibility-window';

/** Main's end of one document's window-update port, as Electron's `MessagePortMain` offers it. */
export interface DesktopWindowUpdatePort {
	close(): void;
	on(event: 'message', listener: (event: { readonly data: unknown }) => void): void;
	postMessage(message: DesktopWindowUpdateMainMessage): void;
	start(): void;
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
	/** Runs `server start --format json` through the bundled Client of window version `version`. */
	readonly startVersionClient: (desktopRoot: string, version: string) => Promise<ServerStartAnswer>;
	/**
	 * Calls back whenever the pointer may have changed; returns what ends the watch, or null when the
	 * folder cannot be watched now.
	 */
	readonly watchPointer: (desktopRoot: string, onChange: () => void) => (() => void) | null;
}

interface ServedPort {
	open: boolean;
	readonly port: DesktopWindowUpdatePort;
}

/**
 * The window's Update control, as Main owns it: the newer window version the pointer names, offered
 * to every document, and a document's or second instance's request to switch to it. A request first
 * checks that the new version's Client joins this Session; when it cannot, a document is told the
 * update waits and a second instance focuses this window.
 */
export class DesktopWindowUpdate {
	#checking = false;
	#disposed = false;
	#offer: string | null = null;
	readonly #options: DesktopWindowUpdateOptions;
	readonly #ports = new Set<ServedPort>();
	#reading: Promise<void> | null = null;
	#rereadRequested = false;
	#stopWatching: (() => void) | undefined;

	constructor(options: DesktopWindowUpdateOptions) {
		this.#options = options;
	}

	/** Whether this window can ever be offered a newer version: only from a release store's folder. */
	get offersUpdates(): boolean {
		return this.#options.desktopRoot !== null;
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
			if (answer?.negotiatedCapabilities.includes(COMPATIBILITY_WINDOW) !== true) {
				this.#recordWaiting(answer);
				return false;
			}
			this.#options.recordEvent('info', 'window-update-joinable');
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
	 * Serves one document's port: the current offer at once and every change, and its requests. A
	 * document is served once a Session start has answered, and on macOS the first start of a window
	 * with no store makes the window store then, so a pointer folder not yet followed is tried again.
	 */
	serve(port: DesktopWindowUpdatePort): { close(): void } {
		const served: ServedPort = { open: true, port };
		this.#ports.add(served);
		port.on('message', ({ data }) => {
			const message = DesktopWindowUpdateRendererMessageSchema.safeParse(data);
			if (!served.open || !message.success) return;
			if (message.data.type === 'desktop-window-update-refresh') {
				void this.refresh();
				return;
			}
			void this.#request(served, message.data);
		});
		port.start();
		port.postMessage({ type: 'desktop-window-update-offered', version: this.#offer });
		this.#follow();
		return {
			close: () => {
				if (!served.open) return;
				served.open = false;
				this.#ports.delete(served);
				port.close();
			},
		};
	}

	dispose(): void {
		this.#disposed = true;
		this.#stopWatching?.();
		this.#stopWatching = undefined;
		for (const served of this.#ports) {
			served.open = false;
			served.port.close();
		}
		this.#ports.clear();
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
			if (answer?.negotiatedCapabilities.includes(COMPATIBILITY_WINDOW) === true) {
				this.#options.recordEvent('info', 'window-update-joinable');
				const started = await this.#options.onJoinable(version, place).catch(() => false);
				if (started === false && served.open) {
					served.port.postMessage({ type: 'desktop-window-update-failed' });
				}
				return;
			}
			this.#recordWaiting(answer);
			if (served.open) served.port.postMessage({ type: 'desktop-window-update-waiting' });
		} finally {
			this.#checking = false;
		}
	}
}
