import {
	BROWSER_AUTHENTICATION_SESSION_PATH,
	BrowserAuthenticationErrorResponseSchema,
	type BrowserAuthenticationErrorCode,
	type PersonalAccessToken,
} from '@orchbee/remote-client-protocol';

export class BrowserAuthenticationClientError extends Error {
	readonly code: BrowserAuthenticationErrorCode | 'unavailable';

	constructor(code: BrowserAuthenticationErrorCode | 'unavailable') {
		super(`Browser authentication failed with ${code}.`);
		this.name = 'BrowserAuthenticationClientError';
		this.code = code;
	}
}

export interface BrowserAuthenticationClientPort {
	authenticate(personalAccessToken: PersonalAccessToken): Promise<void>;
}

export class BrowserAuthenticationClient implements BrowserAuthenticationClientPort {
	readonly #fetch: typeof globalThis.fetch;

	constructor(fetchImplementation: typeof globalThis.fetch = globalThis.fetch) {
		this.#fetch = fetchImplementation;
	}

	async authenticate(personalAccessToken: PersonalAccessToken): Promise<void> {
		let response: Response;
		try {
			response = await this.#fetch(BROWSER_AUTHENTICATION_SESSION_PATH, {
				body: JSON.stringify({ personalAccessToken }),
				credentials: 'same-origin',
				headers: { 'Content-Type': 'application/json' },
				method: 'POST',
			});
		} catch {
			throw new BrowserAuthenticationClientError('unavailable');
		}
		if (response.status === 204) return;
		let body: unknown;
		try {
			body = await response.json();
		} catch {
			throw new BrowserAuthenticationClientError('unavailable');
		}
		const error = BrowserAuthenticationErrorResponseSchema.safeParse(body);
		throw new BrowserAuthenticationClientError(
			error.success ? error.data.error.code : 'unavailable',
		);
	}
}
