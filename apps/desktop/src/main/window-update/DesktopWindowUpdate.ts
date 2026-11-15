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
	 * the asking document shows. Further requests wait until it settles.
	 */
	readonly onJoinable: (version: string, place: DesktopRelaunchPlace | null) => Promise<unknown>;
	readonly ownVersion: string;
	/** The active window version the pointer names, or null. */
	readonly readPointer: (desktopRoot: string) => Promise<string | null>;
	readonly recordEvent: DesktopDiagnosticRecorder;
	/** Runs `server start --format json` through the bundled Client of window version `version`. */
	readonly startVersionClient: (desktopRoot: string, version: string) => Promise<ServerStartAnswer>;
	/** Calls back whenever the pointer may have changed; returns what ends the watch. */
	readonly watchPointer: (desktopRoot: string, onChange: () => void) => () => void;
}

interface ServedPort {
	open: boolean;
	readonly port: DesktopWindowUpdatePort;
}

/**
 * The window's Update control, as Main owns it: the newer window version the pointer names, offered
 * to every document, and a document's request to switch to it. A request first checks that the new
 * version's Client joins this Session; when it cannot, the document is told the update waits.
 */
export class DesktopWindowUpdate {
	#checking = false;
	#disposed = false;
	#offer: string | null = null;
	readonly #options: DesktopWindowUpdateOptions;
	readonly #ports = new Set<ServedPort>();
	#reading = false;
	#rereadRequested = false;
	#stopWatching: (() => void) | undefined;

	constructor(options: DesktopWindowUpdateOptions) {
		this.#options = options;
	}

	/** Reads the pointer and follows its changes; a window outside a version folder offers nothing. */
	start(): void {
		const root = this.#options.desktopRoot;
		if (root === null || this.#disposed || this.#stopWatching !== undefined) return;
		this.#stopWatching = this.#options.watchPointer(root, () => this.#refresh(root));
		this.#refresh(root);
	}

	/** Serves one document's port: the current offer at once and every change, and its requests. */
	serve(port: DesktopWindowUpdatePort): { close(): void } {
		const served: ServedPort = { open: true, port };
		this.#ports.add(served);
		port.on('message', ({ data }) => {
			const message = DesktopWindowUpdateRendererMessageSchema.safeParse(data);
			if (served.open && message.success) void this.#request(served, message.data);
		});
		port.start();
		port.postMessage({ type: 'desktop-window-update-offered', version: this.#offer });
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

	/** One read at a time; a change seen during a read reads again once it ends. */
	#refresh(root: string): void {
		if (this.#reading) {
			this.#rereadRequested = true;
			return;
		}
		this.#reading = true;
		void this.#read(root);
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
		this.#reading = false;
	}

	#setOffer(offer: string | null): void {
		if (offer === this.#offer) return;
		this.#offer = offer;
		for (const served of this.#ports) {
			served.port.postMessage({ type: 'desktop-window-update-offered', version: offer });
		}
	}

	/**
	 * A request for anything but the current offer, or one made while a check or its switch runs, does
	 * nothing: the offer the page shows follows Main's, and one check answers every click made during
	 * it.
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
				await this.#options.onJoinable(version, place).catch(() => undefined);
				return;
			}
			if (answer === null || answer.negotiatedCapabilities.length === 0) {
				this.#options.recordEvent('info', 'window-update-waiting', {
					reason: answer?.restore.kind === 'refused' ? 'start-refused' : 'no-capabilities',
				});
			} else {
				this.#options.recordEvent('info', 'window-update-waiting', {
					reason: 'capability-missing',
				});
			}
			if (served.open) served.port.postMessage({ type: 'desktop-window-update-waiting' });
		} finally {
			this.#checking = false;
		}
	}
}
