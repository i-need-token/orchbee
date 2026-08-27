import {
	DesktopBrowserAccessMainEventSchema,
	DesktopBrowserAccessRendererCommandSchema,
	DesktopBrowserAccessRequestIdSchema,
	type DesktopBrowserAccessFailureCode,
	type DesktopBrowserAccessMode,
	type DesktopBrowserAccessRendererCommand,
	type DesktopBrowserAccessRequestId,
	type DesktopBrowserAccessSnapshot,
} from '@orchbee/desktop-client-protocol';
import type { PersonalAccessToken } from '@orchbee/remote-client-protocol';

import { createBrowserRandomIdentifier } from '../../../browser-random-identifier';

interface PendingSnapshotRequest {
	readonly kind: 'snapshot';
	reject(error: DesktopBrowserAccessClientError): void;
	resolve(snapshot: DesktopBrowserAccessSnapshot): void;
}

interface PendingPersonalAccessTokenRequest {
	readonly kind: 'personal-access-token';
	reject(error: DesktopBrowserAccessClientError): void;
	resolve(result: DesktopPersonalAccessTokenIssue): void;
}

type PendingRequest = PendingPersonalAccessTokenRequest | PendingSnapshotRequest;

export interface DesktopPersonalAccessTokenIssue {
	readonly personalAccessToken: PersonalAccessToken;
	readonly snapshot: DesktopBrowserAccessSnapshot;
}

class DesktopBrowserAccessClientError extends Error {
	readonly code: DesktopBrowserAccessFailureCode;
	readonly snapshot: DesktopBrowserAccessSnapshot;

	constructor(code: DesktopBrowserAccessFailureCode, snapshot: DesktopBrowserAccessSnapshot) {
		super(`Desktop Browser access failed with ${code}.`);
		this.name = 'DesktopBrowserAccessClientError';
		this.code = code;
		this.snapshot = snapshot;
	}
}

export interface DesktopBrowserAccessClientPort {
	copyAccessLink(origin: string): Promise<DesktopBrowserAccessSnapshot>;
	dispose(): void;
	getState(): Promise<DesktopBrowserAccessSnapshot>;
	issuePersonalAccessToken(): Promise<DesktopPersonalAccessTokenIssue>;
	openLocalBrowser(): Promise<DesktopBrowserAccessSnapshot>;
	start(mode: DesktopBrowserAccessMode): Promise<DesktopBrowserAccessSnapshot>;
	stop(): Promise<DesktopBrowserAccessSnapshot>;
}

export class DesktopBrowserAccessClient implements DesktopBrowserAccessClientPort {
	readonly #createRequestId: () => DesktopBrowserAccessRequestId;
	#disposed = false;
	readonly #pending = new Map<DesktopBrowserAccessRequestId, PendingRequest>();
	readonly #port: MessagePort;
	#sequence = 0;
	#snapshot: DesktopBrowserAccessSnapshot = {
		access: { phase: 'stopped' },
		personalAccessToken: 'missing',
	};

	constructor(port: MessagePort, createRequestId: () => string = defaultRequestId) {
		this.#port = port;
		this.#createRequestId = () =>
			DesktopBrowserAccessRequestIdSchema.parse(`${createRequestId()}:${++this.#sequence}`);
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	getState(): Promise<DesktopBrowserAccessSnapshot> {
		return this.#request((requestId) => ({
			requestId,
			type: 'desktop-browser-access-state-requested',
		}));
	}

	start(mode: DesktopBrowserAccessMode): Promise<DesktopBrowserAccessSnapshot> {
		return this.#request((requestId) => ({
			mode,
			requestId,
			type: 'desktop-browser-access-start-requested',
		}));
	}

	openLocalBrowser(): Promise<DesktopBrowserAccessSnapshot> {
		return this.#request((requestId) => ({
			requestId,
			type: 'desktop-browser-access-open-requested',
		}));
	}

	copyAccessLink(origin: string): Promise<DesktopBrowserAccessSnapshot> {
		return this.#request((requestId) => ({
			origin,
			requestId,
			type: 'desktop-browser-access-copy-link-requested',
		}));
	}

	stop(): Promise<DesktopBrowserAccessSnapshot> {
		return this.#request((requestId) => ({
			requestId,
			type: 'desktop-browser-access-stop-requested',
		}));
	}

	issuePersonalAccessToken(): Promise<DesktopPersonalAccessTokenIssue> {
		if (this.#disposed) {
			return Promise.reject(
				new DesktopBrowserAccessClientError('desktop-quiescing', this.#snapshot),
			);
		}
		const requestId = this.#createRequestId();
		const command = DesktopBrowserAccessRendererCommandSchema.parse({
			requestId,
			type: 'desktop-browser-access-personal-access-token-issue-requested',
		});
		const result = new Promise<DesktopPersonalAccessTokenIssue>((resolve, reject) => {
			this.#pending.set(requestId, { kind: 'personal-access-token', reject, resolve });
		});
		this.#postCommand(command);
		return result;
	}

	dispose(): void {
		this.#retire('desktop-quiescing');
	}

	#request(
		createCommand: (
			requestId: DesktopBrowserAccessRequestId,
		) => DesktopBrowserAccessRendererCommand,
	): Promise<DesktopBrowserAccessSnapshot> {
		if (this.#disposed) {
			return Promise.reject(
				new DesktopBrowserAccessClientError('desktop-quiescing', this.#snapshot),
			);
		}
		const requestId = this.#createRequestId();
		const command = DesktopBrowserAccessRendererCommandSchema.parse(createCommand(requestId));
		const result = new Promise<DesktopBrowserAccessSnapshot>((resolve, reject) => {
			this.#pending.set(requestId, { kind: 'snapshot', reject, resolve });
		});
		this.#postCommand(command);
		return result;
	}

	#postCommand(command: DesktopBrowserAccessRendererCommand): void {
		try {
			this.#port.postMessage(command);
		} catch {
			this.#retire('gateway-unavailable');
		}
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || event.ports.length !== 0) {
			this.#retire('gateway-unavailable');
			return;
		}
		const parsed = DesktopBrowserAccessMainEventSchema.safeParse(event.data);
		if (!parsed.success) {
			this.#retire('gateway-unavailable');
			return;
		}
		const publication = parsed.data;
		const pending = this.#pending.get(publication.requestId);
		if (pending === undefined) {
			this.#retire('gateway-unavailable');
			return;
		}
		this.#pending.delete(publication.requestId);
		this.#snapshot = publication.snapshot;
		if (publication.type === 'desktop-browser-access-failed') {
			pending.reject(new DesktopBrowserAccessClientError(publication.code, publication.snapshot));
			return;
		}
		if (publication.type === 'desktop-browser-access-completed' && pending.kind === 'snapshot') {
			pending.resolve(publication.snapshot);
			return;
		}
		if (
			publication.type === 'desktop-browser-access-personal-access-token-issued' &&
			pending.kind === 'personal-access-token'
		) {
			pending.resolve({
				personalAccessToken: publication.personalAccessToken,
				snapshot: publication.snapshot,
			});
			return;
		}
		this.#retire('gateway-unavailable');
	};

	readonly #handleMessageError = (): void => {
		this.#retire('gateway-unavailable');
	};

	#retire(code: DesktopBrowserAccessFailureCode): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		this.#port.close();
		for (const pending of this.#pending.values()) {
			pending.reject(new DesktopBrowserAccessClientError(code, this.#snapshot));
		}
		this.#pending.clear();
	}
}

function defaultRequestId(): string {
	return `browser-access:${createBrowserRandomIdentifier()}`;
}
