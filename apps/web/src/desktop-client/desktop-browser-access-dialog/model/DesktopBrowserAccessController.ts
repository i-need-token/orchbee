import {
	DesktopBrowserAccessFailureCodeSchema,
	DesktopBrowserAccessSnapshotSchema,
	type DesktopBrowserAccessFailureCode,
	type DesktopBrowserAccessMode,
	type DesktopBrowserAccessSnapshot,
} from '@orchbee/desktop-client-protocol';
import type { PersonalAccessToken } from '@orchbee/remote-client-protocol';

import type { DesktopBrowserAccessClientPort } from './DesktopBrowserAccessClient';

export type DesktopBrowserAccessPendingAction =
	| 'copy-link'
	| 'copy-personal-access-token'
	| 'issue-personal-access-token'
	| 'open-browser'
	| 'refresh'
	| 'start-local-network'
	| 'start-loopback'
	| 'stop';

export interface DesktopBrowserAccessDialogState {
	readonly copiedOrigin: string | null;
	readonly copiedPersonalAccessToken: boolean;
	readonly error: DesktopBrowserAccessFailureCode | null;
	readonly issuedPersonalAccessToken: PersonalAccessToken | null;
	readonly open: boolean;
	readonly pendingAction: DesktopBrowserAccessPendingAction | null;
	readonly resetConfirmationOpen: boolean;
	readonly snapshot: DesktopBrowserAccessSnapshot;
}

type Listener = () => void;

export interface DesktopBrowserAccessDialogController {
	close(): void;
	cancelPersonalAccessTokenReset(): void;
	confirmPersonalAccessTokenReset(): void;
	copyAccessLink(origin: string): void;
	copyPersonalAccessToken(): void;
	dispose(): void;
	getSnapshot(): DesktopBrowserAccessDialogState;
	open(): void;
	openLocalBrowser(): void;
	requestPersonalAccessTokenIssue(): void;
	start(mode: DesktopBrowserAccessMode): void;
	stop(): void;
	subscribe(listener: Listener): () => void;
}

export class DesktopBrowserAccessController implements DesktopBrowserAccessDialogController {
	readonly #client: DesktopBrowserAccessClientPort;
	#disposed = false;
	readonly #listeners = new Set<Listener>();
	#operationGeneration = 0;
	readonly #writeClipboardText: (text: string) => Promise<void>;
	#state: DesktopBrowserAccessDialogState = {
		copiedOrigin: null,
		copiedPersonalAccessToken: false,
		error: null,
		issuedPersonalAccessToken: null,
		open: false,
		pendingAction: null,
		resetConfirmationOpen: false,
		snapshot: { access: { phase: 'stopped' }, personalAccessToken: 'missing' },
	};

	constructor(
		client: DesktopBrowserAccessClientPort,
		writeClipboardText: (text: string) => Promise<void> = () => Promise.reject(new Error()),
	) {
		this.#client = client;
		this.#writeClipboardText = writeClipboardText;
	}

	getSnapshot = (): DesktopBrowserAccessDialogState => this.#state;

	subscribe = (listener: Listener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	open(): void {
		if (this.#disposed) return;
		this.#setState({
			...this.#state,
			copiedPersonalAccessToken: false,
			error: null,
			issuedPersonalAccessToken: null,
			open: true,
			resetConfirmationOpen: false,
		});
		void this.#run('refresh', () => this.#client.getState());
	}

	close(): void {
		if (this.#disposed) return;
		this.#setState({
			...this.#state,
			copiedOrigin: null,
			copiedPersonalAccessToken: false,
			error: null,
			issuedPersonalAccessToken: null,
			open: false,
			resetConfirmationOpen: false,
		});
	}

	start(mode: DesktopBrowserAccessMode): void {
		void this.#run(mode === 'loopback' ? 'start-loopback' : 'start-local-network', () =>
			this.#client.start(mode),
		);
	}

	openLocalBrowser(): void {
		void this.#run('open-browser', async () => {
			if (this.#state.snapshot.access.phase !== 'ready') await this.#client.start('loopback');
			return this.#client.openLocalBrowser();
		});
	}

	copyAccessLink(origin: string): void {
		void this.#run('copy-link', async () => {
			const snapshot = await this.#client.copyAccessLink(origin);
			this.#setState({ ...this.#state, copiedOrigin: origin, snapshot });
			return snapshot;
		});
	}

	requestPersonalAccessTokenIssue(): void {
		if (this.#disposed || this.#state.pendingAction !== null) return;
		if (this.#state.snapshot.personalAccessToken === 'configured') {
			this.#setState({ ...this.#state, resetConfirmationOpen: true });
			return;
		}
		void this.#issuePersonalAccessToken();
	}

	cancelPersonalAccessTokenReset(): void {
		if (this.#disposed || this.#state.pendingAction !== null) return;
		this.#setState({ ...this.#state, resetConfirmationOpen: false });
	}

	confirmPersonalAccessTokenReset(): void {
		if (
			this.#disposed ||
			this.#state.pendingAction !== null ||
			!this.#state.resetConfirmationOpen
		) {
			return;
		}
		this.#setState({ ...this.#state, resetConfirmationOpen: false });
		void this.#issuePersonalAccessToken();
	}

	copyPersonalAccessToken(): void {
		const token = this.#state.issuedPersonalAccessToken;
		if (this.#disposed || this.#state.pendingAction !== null || token === null) return;
		const generation = ++this.#operationGeneration;
		this.#setState({
			...this.#state,
			copiedPersonalAccessToken: false,
			error: null,
			pendingAction: 'copy-personal-access-token',
		});
		void this.#writeClipboardText(token).then(
			() => {
				if (this.#disposed || generation !== this.#operationGeneration) return;
				this.#setState({
					...this.#state,
					copiedPersonalAccessToken: true,
					pendingAction: null,
				});
			},
			() => {
				if (this.#disposed || generation !== this.#operationGeneration) return;
				this.#setState({
					...this.#state,
					error: 'clipboard-write-failed',
					pendingAction: null,
				});
			},
		);
	}

	stop(): void {
		void this.#run('stop', () => this.#client.stop());
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#operationGeneration += 1;
		this.#state = {
			...this.#state,
			copiedPersonalAccessToken: false,
			issuedPersonalAccessToken: null,
			open: false,
			pendingAction: null,
			resetConfirmationOpen: false,
		};
		this.#listeners.clear();
		this.#client.dispose();
	}

	async #run(
		action: DesktopBrowserAccessPendingAction,
		operation: () => Promise<DesktopBrowserAccessSnapshot>,
	): Promise<void> {
		if (this.#disposed || this.#state.pendingAction !== null) return;
		const generation = ++this.#operationGeneration;
		this.#setState({
			...this.#state,
			copiedOrigin: action === 'copy-link' ? this.#state.copiedOrigin : null,
			copiedPersonalAccessToken: false,
			error: null,
			pendingAction: action,
		});
		try {
			const snapshot = await operation();
			if (this.#disposed || generation !== this.#operationGeneration) return;
			this.#setState({ ...this.#state, pendingAction: null, snapshot });
		} catch (error: unknown) {
			if (this.#disposed || generation !== this.#operationGeneration) return;
			const clientError = parseClientError(error, this.#state.snapshot);
			this.#setState({
				...this.#state,
				error: clientError.code,
				pendingAction: null,
				snapshot: clientError.snapshot,
			});
		}
	}

	async #issuePersonalAccessToken(): Promise<void> {
		if (this.#disposed || this.#state.pendingAction !== null) return;
		const generation = ++this.#operationGeneration;
		this.#setState({
			...this.#state,
			copiedOrigin: null,
			copiedPersonalAccessToken: false,
			error: null,
			issuedPersonalAccessToken: null,
			pendingAction: 'issue-personal-access-token',
		});
		try {
			const result = await this.#client.issuePersonalAccessToken();
			if (this.#disposed || generation !== this.#operationGeneration) return;
			this.#setState({
				...this.#state,
				issuedPersonalAccessToken: result.personalAccessToken,
				pendingAction: null,
				snapshot: result.snapshot,
			});
		} catch (error: unknown) {
			if (this.#disposed || generation !== this.#operationGeneration) return;
			const clientError = parseClientError(error, this.#state.snapshot);
			this.#setState({
				...this.#state,
				error: clientError.code,
				pendingAction: null,
				snapshot: clientError.snapshot,
			});
		}
	}

	#setState(state: DesktopBrowserAccessDialogState): void {
		this.#state = state;
		for (const listener of this.#listeners) listener();
	}
}

function parseClientError(
	error: unknown,
	fallbackSnapshot: DesktopBrowserAccessSnapshot,
): {
	readonly code: DesktopBrowserAccessFailureCode;
	readonly snapshot: DesktopBrowserAccessSnapshot;
} {
	if (typeof error !== 'object' || error === null) {
		return { code: 'gateway-unavailable', snapshot: fallbackSnapshot };
	}
	const code = DesktopBrowserAccessFailureCodeSchema.safeParse(
		'code' in error ? error.code : undefined,
	);
	const snapshot = DesktopBrowserAccessSnapshotSchema.safeParse(
		'snapshot' in error ? error.snapshot : undefined,
	);
	return {
		code: code.success ? code.data : 'gateway-unavailable',
		snapshot: snapshot.success ? snapshot.data : fallbackSnapshot,
	};
}
