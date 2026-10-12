import {
	DesktopUserPreferencesCommandSchema,
	DesktopUserPreferencesEventSchema,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';

import type {
	DesktopUserPreferencesController,
	DesktopUserPreferencesObservation,
} from './DesktopUserPreferencesController';

export class DesktopUserPreferencesBroker {
	readonly #port: MessagePortMain;
	readonly #controller: DesktopUserPreferencesController;
	#removeListener: (() => void) | undefined;
	#disposed = false;
	#active = false;
	#lastRequestId = 0;

	constructor(port: MessagePortMain, controller: DesktopUserPreferencesController) {
		this.#port = port;
		this.#controller = controller;
	}

	start(): void {
		if (this.#disposed || this.#removeListener !== undefined) return;
		this.#removeListener = this.#controller.subscribe((observation) => {
			if (!this.#active) this.#post(null, observation);
		});
		this.#port.on('message', this.#handleMessage);
		this.#port.on('close', this.#handleClose);
		this.#port.start();
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#removeListener?.();
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		this.#port.close();
	}

	readonly #handleClose = (): void => {
		this.dispose();
	};
	readonly #handleMessage = (event: MessageEvent): void => {
		const parsed = DesktopUserPreferencesCommandSchema.safeParse(event.data);
		if (
			event.ports.length !== 0 ||
			!parsed.success ||
			parsed.data.requestId <= this.#lastRequestId
		) {
			this.dispose();
			return;
		}
		const { requestId, request } = parsed.data;
		this.#lastRequestId = requestId;
		if (this.#active) {
			this.#post(requestId, {
				response: { kind: 'refused', reason: 'busy' },
				applicationIssue: null,
				legacyIssue: false,
			});
			return;
		}
		this.#active = true;
		void this.#controller
			.execute(request)
			.then((observation) => this.#post(requestId, observation))
			.catch(() => {
				this.#post(requestId, {
					response:
						request.operation === 'read'
							? { kind: 'refused', reason: 'unavailable' }
							: { kind: 'outcome-unknown' },
					applicationIssue: null,
					legacyIssue: false,
				});
			})
			.finally(() => {
				this.#active = false;
			});
	};

	#post(requestId: number | null, observation: DesktopUserPreferencesObservation): void {
		if (this.#disposed) return;
		try {
			this.#port.postMessage(
				DesktopUserPreferencesEventSchema.parse({
					type: 'desktop-user-preferences-observed',
					requestId,
					...observation,
				}),
			);
		} catch {
			this.dispose();
		}
	}
}
