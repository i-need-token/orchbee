import { createHash, randomBytes } from 'node:crypto';

import {
	PersonalAccessTokenSchema,
	PersonalAccessTokenVerifierSchema,
	type PersonalAccessToken,
	type PersonalAccessTokenVerifier,
} from '@orchbee/remote-client-protocol';

export interface DesktopPersonalAccessTokenStore {
	load(): Promise<PersonalAccessTokenVerifier | null>;
	save(verifier: PersonalAccessTokenVerifier): Promise<void>;
}

interface DesktopPersonalAccessTokenControllerDependencies {
	readonly createToken: () => PersonalAccessToken;
	readonly deriveVerifier: (token: PersonalAccessToken) => PersonalAccessTokenVerifier;
}

const defaultDependencies: DesktopPersonalAccessTokenControllerDependencies = {
	createToken: () => PersonalAccessTokenSchema.parse(randomBytes(32).toString('base64url')),
	deriveVerifier: (token) =>
		PersonalAccessTokenVerifierSchema.parse(
			createHash('sha256').update(token, 'utf8').digest('base64url'),
		),
};

export class DesktopPersonalAccessTokenController {
	readonly #dependencies: DesktopPersonalAccessTokenControllerDependencies;
	#initialized = false;
	readonly #store: DesktopPersonalAccessTokenStore;
	#verifier: PersonalAccessTokenVerifier | null = null;

	constructor(
		store: DesktopPersonalAccessTokenStore,
		dependencies: Partial<DesktopPersonalAccessTokenControllerDependencies> = {},
	) {
		this.#store = store;
		this.#dependencies = { ...defaultDependencies, ...dependencies };
	}

	async initialize(): Promise<'configured' | 'missing'> {
		if (!this.#initialized) {
			this.#verifier = await this.#store.load();
			this.#initialized = true;
		}
		return this.getStatus();
	}

	getStatus(): 'configured' | 'missing' {
		this.#requireInitialized();
		return this.#verifier === null ? 'missing' : 'configured';
	}

	getVerifier(): PersonalAccessTokenVerifier | null {
		this.#requireInitialized();
		return this.#verifier;
	}

	async issue(): Promise<PersonalAccessToken> {
		this.#requireInitialized();
		const token = this.#dependencies.createToken();
		const verifier = this.#dependencies.deriveVerifier(token);
		await this.#store.save(verifier);
		this.#verifier = verifier;
		return token;
	}

	#requireInitialized(): void {
		if (!this.#initialized) {
			throw new Error('The Personal Access Token controller is not initialized.');
		}
	}
}
