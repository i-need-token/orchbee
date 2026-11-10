import {
	DesktopBackgroundStatusMainMessageSchema,
	type DesktopBackgroundStatusRendererMessage,
	type DesktopBackgroundStatusSummary,
	type DesktopSessionStopAnswer,
} from '@orchbee/desktop-client-protocol';

type SessionStop = (stopId: string) => Promise<DesktopSessionStopAnswer>;

/**
 * This document's background-status port: what Main's notification-area icon shows while the
 * window is hidden, the request to show the window, which a page cannot make for itself, and the
 * answer to each Session stop Main asks for.
 */
export class DesktopBackgroundStatusClient {
	#disposed = false;
	#reported: DesktopBackgroundStatusSummary | undefined;
	readonly #port: MessagePort;
	#sessionStop: SessionStop | undefined;

	constructor(port: MessagePort) {
		this.#port = port;
		// Main may ask before a Server connection serves stops; such a stop is answered at once.
		port.addEventListener('message', ({ data }) => this.#receive(data));
		port.start();
	}

	/** Serves Main's Session stops with `stop`; without it, each is answered as not sent. */
	serveSessionStop(stop: SessionStop): void {
		this.#sessionStop = stop;
	}

	/** Sends `summary` unless it equals the last one sent. */
	report(summary: DesktopBackgroundStatusSummary): void {
		if (this.#disposed || (this.#reported !== undefined && sameSummary(this.#reported, summary))) {
			return;
		}
		this.#reported = summary;
		this.#post({ summary, type: 'desktop-background-status-changed' });
	}

	/** Asks Main to show the window and bring it forward, as a notification click does. */
	requestWindow(): void {
		if (!this.#disposed) this.#post({ type: 'desktop-background-window-requested' });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.close();
	}

	#post(message: DesktopBackgroundStatusRendererMessage): void {
		this.#port.postMessage(message);
	}

	#receive(data: unknown): void {
		const message = DesktopBackgroundStatusMainMessageSchema.safeParse(data);
		if (this.#disposed || !message.success) return;
		const { stopId } = message.data;
		const stop = this.#sessionStop;
		// A stop that failed to settle may still have been recorded, so its status decides.
		const answer: Promise<DesktopSessionStopAnswer> =
			stop === undefined
				? Promise.resolve('not-sent')
				: stop(stopId).catch((): DesktopSessionStopAnswer => 'unknown');
		void answer.then((settled) => {
			if (!this.#disposed) {
				this.#post({ answer: settled, stopId, type: 'desktop-session-stop-answered' });
			}
		});
	}
}

function sameSummary(
	left: DesktopBackgroundStatusSummary,
	right: DesktopBackgroundStatusSummary,
): boolean {
	return (
		left.attention === right.attention &&
		left.doneUnseen === right.doneUnseen &&
		left.failed === right.failed &&
		left.runningTerminals === right.runningTerminals &&
		left.workingAgents === right.workingAgents
	);
}
