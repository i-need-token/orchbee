import {
	PersonalAccessTokenSchema,
	type BrowserAuthenticationErrorCode,
} from '@orchbee/remote-client-protocol';

import {
	BrowserAuthenticationClientError,
	type BrowserAuthenticationClientPort,
} from './browser-authentication-client';

export type BrowserAuthenticationViewError =
	| BrowserAuthenticationErrorCode
	| 'required'
	| 'unavailable';

export interface BrowserAuthenticationState {
	readonly error: BrowserAuthenticationViewError | null;
	readonly pending: boolean;
	readonly personalAccessToken: string;
}

type Listener = () => void;

export interface BrowserAuthenticationControllerPort {
	dispose(): void;
	getSnapshot(): BrowserAuthenticationState;
	setPersonalAccessToken(value: string): void;
	submit(): void;
	subscribe(listener: Listener): () => void;
}

export class BrowserAuthenticationController implements BrowserAuthenticationControllerPort {
	readonly #client: BrowserAuthenticationClientPort;
	#disposed = false;
	readonly #listeners = new Set<Listener>();
	readonly #navigate: (target: string) => void;
	readonly #returnTarget: string;
	#state: BrowserAuthenticationState = {
		error: null,
		pending: false,
		personalAccessToken: '',
	};

	constructor(options: {
		readonly client: BrowserAuthenticationClientPort;
		readonly navigate: (target: string) => void;
		readonly returnTarget: string;
	}) {
		this.#client = options.client;
		this.#navigate = options.navigate;
		this.#returnTarget = options.returnTarget;
	}

	getSnapshot = (): BrowserAuthenticationState => this.#state;

	subscribe = (listener: Listener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	setPersonalAccessToken(value: string): void {
		if (this.#disposed || this.#state.pending) return;
		this.#setState({ error: null, pending: false, personalAccessToken: value });
	}

	submit(): void {
		if (this.#disposed || this.#state.pending) return;
		const value = this.#state.personalAccessToken.trim();
		if (value === '') {
			this.#setState({ ...this.#state, error: 'required' });
			return;
		}
		const personalAccessToken = PersonalAccessTokenSchema.safeParse(value);
		if (!personalAccessToken.success) {
			this.#setState({ ...this.#state, error: 'invalid-request' });
			return;
		}
		this.#setState({ error: null, pending: true, personalAccessToken: '' });
		void this.#client.authenticate(personalAccessToken.data).then(
			() => {
				if (this.#disposed) return;
				this.#state = { error: null, pending: false, personalAccessToken: '' };
				this.#navigate(this.#returnTarget);
			},
			(error: unknown) => {
				if (this.#disposed) return;
				this.#setState({
					error: error instanceof BrowserAuthenticationClientError ? error.code : 'unavailable',
					pending: false,
					personalAccessToken: '',
				});
			},
		);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#state = { error: null, pending: false, personalAccessToken: '' };
		this.#listeners.clear();
	}

	#setState(state: BrowserAuthenticationState): void {
		this.#state = state;
		for (const listener of this.#listeners) listener();
	}
}

export function resolveBrowserAuthenticationReturnTarget(search: string): string {
	const searchParameters = new URLSearchParams(search);
	const targets = searchParameters.getAll('returnTo');
	if (targets.length !== 1) return '/';
	const candidate = targets[0];
	if (candidate === undefined || !candidate.startsWith('/') || candidate.startsWith('//'))
		return '/';
	try {
		const resolved = new URL(candidate, 'http://orchbee.invalid');
		if (resolved.origin !== 'http://orchbee.invalid' || resolved.pathname === '/login') return '/';
		return `${resolved.pathname}${resolved.search}`;
	} catch {
		return '/';
	}
}
