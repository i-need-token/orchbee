import {
	DesktopBrowserAccessFailureCodeSchema,
	DesktopBrowserAccessStateSchema,
	type DesktopBrowserAccessFailureCode,
	type DesktopBrowserAccessMode,
	type DesktopBrowserAccessState,
} from '@orchbee/desktop-client-protocol';

import type { DesktopBrowserAccessClientPort } from './DesktopBrowserAccessClient';

export type DesktopBrowserAccessPendingAction =
	| 'copy-link'
	| 'open-browser'
	| 'refresh'
	| 'start-local-network'
	| 'start-loopback'
	| 'stop';

export interface DesktopBrowserAccessDialogState {
	readonly copiedOrigin: string | null;
	readonly error: DesktopBrowserAccessFailureCode | null;
	readonly open: boolean;
	readonly pendingAction: DesktopBrowserAccessPendingAction | null;
	readonly runtime: DesktopBrowserAccessState;
}

type Listener = () => void;

export interface DesktopBrowserAccessDialogController {
	close(): void;
	copyAccessLink(origin: string): void;
	dispose(): void;
	getSnapshot(): DesktopBrowserAccessDialogState;
	open(): void;
	openLocalBrowser(): void;
	start(mode: DesktopBrowserAccessMode): void;
	stop(): void;
	subscribe(listener: Listener): () => void;
}

export class DesktopBrowserAccessController implements DesktopBrowserAccessDialogController {
	readonly #client: DesktopBrowserAccessClientPort;
	#disposed = false;
	readonly #listeners = new Set<Listener>();
	#operationGeneration = 0;
	#state: DesktopBrowserAccessDialogState = {
		copiedOrigin: null,
		error: null,
		open: false,
		pendingAction: null,
		runtime: { phase: 'stopped' },
	};

	constructor(client: DesktopBrowserAccessClientPort) {
		this.#client = client;
	}

	getSnapshot = (): DesktopBrowserAccessDialogState => this.#state;

	subscribe = (listener: Listener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	open(): void {
		if (this.#disposed) return;
		this.#setState({ ...this.#state, error: null, open: true });
		void this.#run('refresh', () => this.#client.getState());
	}

	close(): void {
		if (this.#disposed) return;
		this.#setState({ ...this.#state, copiedOrigin: null, error: null, open: false });
	}

	start(mode: DesktopBrowserAccessMode): void {
		void this.#run(mode === 'loopback' ? 'start-loopback' : 'start-local-network', () =>
			this.#client.start(mode),
		);
	}

	openLocalBrowser(): void {
		void this.#run('open-browser', async () => {
			if (this.#state.runtime.phase !== 'ready') await this.#client.start('loopback');
			return this.#client.openLocalBrowser();
		});
	}

	copyAccessLink(origin: string): void {
		void this.#run('copy-link', async () => {
			const state = await this.#client.copyAccessLink(origin);
			this.#setState({ ...this.#state, copiedOrigin: origin, runtime: state });
			return state;
		});
	}

	stop(): void {
		void this.#run('stop', () => this.#client.stop());
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#operationGeneration += 1;
		this.#listeners.clear();
		this.#client.dispose();
	}

	async #run(
		action: DesktopBrowserAccessPendingAction,
		operation: () => Promise<DesktopBrowserAccessState>,
	): Promise<void> {
		if (this.#disposed || this.#state.pendingAction !== null) return;
		const generation = ++this.#operationGeneration;
		this.#setState({
			...this.#state,
			copiedOrigin: action === 'copy-link' ? this.#state.copiedOrigin : null,
			error: null,
			pendingAction: action,
		});
		try {
			const runtime = await operation();
			if (this.#disposed || generation !== this.#operationGeneration) return;
			this.#setState({ ...this.#state, pendingAction: null, runtime });
		} catch (error: unknown) {
			if (this.#disposed || generation !== this.#operationGeneration) return;
			const clientError = parseClientError(error, this.#state.runtime);
			this.#setState({
				...this.#state,
				error: clientError.code,
				pendingAction: null,
				runtime: clientError.state,
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
	fallbackState: DesktopBrowserAccessState,
): { readonly code: DesktopBrowserAccessFailureCode; readonly state: DesktopBrowserAccessState } {
	if (typeof error !== 'object' || error === null) {
		return { code: 'gateway-unavailable', state: fallbackState };
	}
	const code = DesktopBrowserAccessFailureCodeSchema.safeParse(
		'code' in error ? error.code : undefined,
	);
	const state = DesktopBrowserAccessStateSchema.safeParse(
		'state' in error ? error.state : undefined,
	);
	return {
		code: code.success ? code.data : 'gateway-unavailable',
		state: state.success ? state.data : fallbackState,
	};
}
