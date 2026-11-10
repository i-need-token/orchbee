import type {
	DesktopBackgroundStatusRendererMessage,
	DesktopBackgroundStatusSummary,
} from '@orchbee/desktop-client-protocol';

/**
 * This document's background-status port: what Main's notification-area icon shows while the
 * window is hidden, and the request to show the window, which a page cannot make for itself.
 */
export class DesktopBackgroundStatusClient {
	#disposed = false;
	#reported: DesktopBackgroundStatusSummary | undefined;
	readonly #port: MessagePort;

	constructor(port: MessagePort) {
		this.#port = port;
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
