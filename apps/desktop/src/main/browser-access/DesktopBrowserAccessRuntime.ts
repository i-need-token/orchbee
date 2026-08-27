import { randomBytes } from 'node:crypto';
import { networkInterfaces, type NetworkInterfaceInfo } from 'node:os';

import {
	type DesktopBrowserAccessFailureCode,
	type DesktopBrowserAccessMode,
	type DesktopBrowserAccessSnapshot,
	type DesktopBrowserAccessState,
} from '@orchbee/desktop-client-protocol';
import { decodeGatewayBrowserSearch } from '@orchbee/gateway-protocol';
import {
	BrowserSessionSigningKeySchema,
	type BrowserSessionSigningKey,
	type PersonalAccessToken,
	type PersonalAccessTokenVerifier,
} from '@orchbee/remote-client-protocol';
import { clipboard, shell } from 'electron';

import {
	LocalDevelopmentGatewayRuntime,
	type DesktopGatewayClientHandle,
	type LocalDevelopmentGatewayRuntimeOptions,
} from '../gateway';
import { DesktopPersonalAccessTokenController } from './DesktopPersonalAccessTokenController';
import { FileDesktopPersonalAccessTokenStore } from './FileDesktopPersonalAccessTokenStore';
import { LocalBrowserRendererServerRuntime } from './LocalBrowserRendererServerRuntime';
import { resolveDesktopBrowserAccessEndpoints } from './resolveDesktopBrowserAccessEndpoints';

interface DesktopBrowserAccessServerHandle {
	readonly backgroundFailure?: Error | undefined;
	dispose(): Promise<void>;
	start(): Promise<{ readonly port: number }>;
}

interface DesktopBrowserAccessGatewayHandle {
	readonly backgroundFailure?: Error | undefined;
	dispose(): Promise<void>;
	start(): Promise<{
		readonly browserUrl: string;
		readonly browserUrls: readonly { readonly origin: string; readonly url: string }[];
	}>;
}

export interface DesktopBrowserAccessRuntimeOptions {
	readonly gatewayEntryPath: string;
	readonly personalAccessTokenStorePath: string;
	readonly rendererRoot: string;
	readonly serverEntryPath: string;
}

interface DesktopBrowserAccessRuntimeDependencies {
	readonly createSessionSigningKey: () => BrowserSessionSigningKey;
	readonly createGateway: (
		options: LocalDevelopmentGatewayRuntimeOptions,
		openClient: () => Promise<DesktopGatewayClientHandle>,
	) => DesktopBrowserAccessGatewayHandle;
	readonly createServer: (options: {
		readonly authentication: {
			readonly personalAccessTokenVerifier: PersonalAccessTokenVerifier;
			readonly sessionSigningKey: BrowserSessionSigningKey;
		};
		readonly bindHost: '0.0.0.0' | '127.0.0.1';
		readonly entryPath: string;
		readonly rendererRoot: string;
	}) => DesktopBrowserAccessServerHandle;
	readonly getNetworkInterfaces: () => Readonly<
		Record<string, readonly NetworkInterfaceInfo[] | undefined>
	>;
	readonly openClient: () => Promise<DesktopGatewayClientHandle>;
	readonly openExternal: (url: string) => Promise<void>;
	readonly personalAccessTokenController: Pick<
		DesktopPersonalAccessTokenController,
		'getStatus' | 'getVerifier' | 'initialize' | 'issue'
	>;
	readonly writeClipboardText: (text: string) => void;
}

export interface DesktopPersonalAccessTokenIssueResult {
	readonly personalAccessToken: PersonalAccessToken;
	readonly snapshot: DesktopBrowserAccessSnapshot;
}

class DesktopBrowserAccessError extends Error {
	readonly code: DesktopBrowserAccessFailureCode;

	constructor(code: DesktopBrowserAccessFailureCode, message: string, options?: ErrorOptions) {
		super(message, options);
		this.name = 'DesktopBrowserAccessError';
		this.code = code;
	}
}

interface ReadyBrowserAccess {
	readonly gateway: DesktopBrowserAccessGatewayHandle;
	readonly linksByOrigin: ReadonlyMap<string, string>;
	readonly server: DesktopBrowserAccessServerHandle;
	readonly state: Extract<DesktopBrowserAccessState, { readonly phase: 'ready' }>;
}

export class DesktopBrowserAccessRuntime {
	readonly #dependencies: DesktopBrowserAccessRuntimeDependencies;
	#disposePromise: Promise<void> | undefined;
	#disposeRequested = false;
	readonly #options: DesktopBrowserAccessRuntimeOptions;
	#operationTail = Promise.resolve();
	#personalAccessTokenAvailable = false;
	#personalAccessTokenInitialized = false;
	#ready: ReadyBrowserAccess | undefined;
	#state: DesktopBrowserAccessState = { phase: 'stopped' };

	constructor(
		options: DesktopBrowserAccessRuntimeOptions,
		dependencies: Partial<DesktopBrowserAccessRuntimeDependencies> &
			Pick<DesktopBrowserAccessRuntimeDependencies, 'openClient'>,
	) {
		this.#options = options;
		const personalAccessTokenController =
			dependencies.personalAccessTokenController ??
			new DesktopPersonalAccessTokenController(
				new FileDesktopPersonalAccessTokenStore(options.personalAccessTokenStorePath),
			);
		this.#dependencies = {
			createSessionSigningKey: () =>
				BrowserSessionSigningKeySchema.parse(randomBytes(32).toString('base64url')),
			createGateway: (gatewayOptions, openClient) =>
				new LocalDevelopmentGatewayRuntime(gatewayOptions, { openClient }),
			createServer: (serverOptions) => new LocalBrowserRendererServerRuntime(serverOptions),
			getNetworkInterfaces: networkInterfaces,
			openExternal: (url) => shell.openExternal(url),
			personalAccessTokenController,
			writeClipboardText: (text) => clipboard.writeText(text),
			...dependencies,
		};
	}

	initialize(): Promise<DesktopBrowserAccessSnapshot> {
		if (this.#disposeRequested) return Promise.reject(quiescingError());
		return this.#enqueue(async () => {
			try {
				await this.#ensurePersonalAccessTokenInitialized();
			} catch (error: unknown) {
				if (!(error instanceof DesktopBrowserAccessError)) throw error;
				if (error.code !== 'personal-access-token-store-unavailable') throw error;
			}
			return this.getSnapshot();
		});
	}

	getSnapshot(): DesktopBrowserAccessSnapshot {
		return {
			access: this.#state,
			personalAccessToken: this.#personalAccessTokenAvailable
				? this.#dependencies.personalAccessTokenController.getStatus()
				: 'unavailable',
		};
	}

	start(mode: DesktopBrowserAccessMode): Promise<DesktopBrowserAccessSnapshot> {
		if (this.#disposeRequested) return Promise.reject(quiescingError());
		return this.#enqueue(async () => {
			await this.#ensurePersonalAccessTokenInitialized();
			await this.#start(mode);
			return this.getSnapshot();
		});
	}

	stop(): Promise<DesktopBrowserAccessSnapshot> {
		if (this.#disposeRequested) return Promise.reject(quiescingError());
		return this.#enqueue(async () => {
			await this.#stopReady();
			return this.getSnapshot();
		});
	}

	openLocalBrowser(): Promise<DesktopBrowserAccessSnapshot> {
		if (this.#disposeRequested) return Promise.reject(quiescingError());
		return this.#enqueue(async () => {
			const ready = this.#requireReady();
			const url = ready.linksByOrigin.get(ready.state.localOrigin);
			if (url === undefined) throw invalidOriginError();
			try {
				await this.#dependencies.openExternal(url);
			} catch (error: unknown) {
				throw new DesktopBrowserAccessError(
					'browser-open-failed',
					'The system Browser could not open the Orchbee access link.',
					{ cause: error },
				);
			}
			return this.getSnapshot();
		});
	}

	copyAccessLink(origin: string): Promise<DesktopBrowserAccessSnapshot> {
		if (this.#disposeRequested) return Promise.reject(quiescingError());
		return this.#enqueue(async () => {
			const ready = this.#requireReady();
			const url = ready.linksByOrigin.get(origin);
			if (url === undefined) throw invalidOriginError();
			try {
				this.#dependencies.writeClipboardText(url);
			} catch (error: unknown) {
				throw new DesktopBrowserAccessError(
					'clipboard-write-failed',
					'The Orchbee Browser access link could not be copied.',
					{ cause: error },
				);
			}
			return this.getSnapshot();
		});
	}

	issuePersonalAccessToken(): Promise<DesktopPersonalAccessTokenIssueResult> {
		if (this.#disposeRequested) return Promise.reject(quiescingError());
		return this.#enqueue(async () => {
			await this.#ensurePersonalAccessTokenInitialized();
			this.#requirePersonalAccessTokenAvailable();
			await this.#stopReady();
			let personalAccessToken: PersonalAccessToken;
			try {
				personalAccessToken = await this.#dependencies.personalAccessTokenController.issue();
			} catch (error: unknown) {
				throw personalAccessTokenStoreError(error);
			}
			return { personalAccessToken, snapshot: this.getSnapshot() };
		});
	}

	dispose(): Promise<void> {
		if (this.#disposePromise !== undefined) return this.#disposePromise;
		this.#disposeRequested = true;
		this.#disposePromise = this.#enqueue(async () => {
			await this.#stopReady();
		});
		return this.#disposePromise;
	}

	async #start(mode: DesktopBrowserAccessMode): Promise<DesktopBrowserAccessState> {
		if (this.#disposeRequested) throw quiescingError();
		const personalAccessTokenVerifier = this.#requirePersonalAccessTokenVerifier();
		const current = this.#ready;
		if (
			current !== undefined &&
			current.state.mode === mode &&
			current.server.backgroundFailure === undefined &&
			current.gateway.backgroundFailure === undefined
		) {
			return current.state;
		}
		await this.#stopReady();
		if (this.#disposeRequested) throw quiescingError();
		const interfaces = this.#dependencies.getNetworkInterfaces();
		let provisionalBindHost: '0.0.0.0' | '127.0.0.1';
		try {
			provisionalBindHost = resolveDesktopBrowserAccessEndpoints(mode, 1, interfaces).bindHost;
		} catch (error: unknown) {
			throw mapNetworkError(error);
		}
		this.#state = { mode, phase: 'starting' };
		let server: DesktopBrowserAccessServerHandle | undefined;
		let gateway: DesktopBrowserAccessGatewayHandle | undefined;
		let stage: 'gateway' | 'server' = 'server';
		try {
			const sessionSigningKey = this.#dependencies.createSessionSigningKey();
			server = this.#dependencies.createServer({
				authentication: { personalAccessTokenVerifier, sessionSigningKey },
				bindHost: provisionalBindHost,
				entryPath: this.#options.serverEntryPath,
				rendererRoot: this.#options.rendererRoot,
			});
			const serverStart = await server.start();
			if (this.#disposeRequested) throw quiescingError();
			let endpoints: ReturnType<typeof resolveDesktopBrowserAccessEndpoints>;
			try {
				endpoints = resolveDesktopBrowserAccessEndpoints(mode, serverStart.port, interfaces);
			} catch (error: unknown) {
				throw mapNetworkError(error);
			}
			const origins = [endpoints.localOrigin, ...endpoints.networkOrigins];
			stage = 'gateway';
			gateway = this.#dependencies.createGateway(
				{
					bindHost: endpoints.bindHost,
					entryPath: this.#options.gatewayEntryPath,
					rendererOrigins: origins,
					sessionSigningKey,
				},
				this.#dependencies.openClient,
			);
			const gatewayStart = await gateway.start();
			if (this.#disposeRequested) throw quiescingError();
			const linksByOrigin = validateBrowserLinks(origins, gatewayStart.browserUrls);
			const state = {
				localOrigin: endpoints.localOrigin,
				mode,
				networkOrigins: [...endpoints.networkOrigins],
				phase: 'ready',
			} as const satisfies DesktopBrowserAccessState;
			this.#ready = { gateway, linksByOrigin, server, state };
			this.#state = state;
			return state;
		} catch (error: unknown) {
			const failures: unknown[] = [];
			if (gateway !== undefined) {
				try {
					await gateway.dispose();
				} catch (cleanupError: unknown) {
					failures.push(cleanupError);
				}
			}
			if (server !== undefined) {
				try {
					await server.dispose();
				} catch (cleanupError: unknown) {
					failures.push(cleanupError);
				}
			}
			this.#state = { phase: 'stopped' };
			if (error instanceof DesktopBrowserAccessError) throw error;
			throw new DesktopBrowserAccessError(
				stage === 'server' ? 'server-unavailable' : 'gateway-unavailable',
				`Orchbee Browser access ${stage} startup failed.`,
				{ cause: failures.length === 0 ? error : new AggregateError([error, ...failures]) },
			);
		}
	}

	async #ensurePersonalAccessTokenInitialized(): Promise<void> {
		if (this.#personalAccessTokenInitialized) {
			this.#requirePersonalAccessTokenAvailable();
			return;
		}
		this.#personalAccessTokenInitialized = true;
		try {
			await this.#dependencies.personalAccessTokenController.initialize();
			this.#personalAccessTokenAvailable = true;
		} catch (error: unknown) {
			this.#personalAccessTokenAvailable = false;
			throw personalAccessTokenStoreError(error);
		}
	}

	#requirePersonalAccessTokenAvailable(): void {
		if (!this.#personalAccessTokenAvailable) throw personalAccessTokenStoreError();
	}

	#requirePersonalAccessTokenVerifier(): PersonalAccessTokenVerifier {
		this.#requirePersonalAccessTokenAvailable();
		const verifier = this.#dependencies.personalAccessTokenController.getVerifier();
		if (verifier === null) {
			throw new DesktopBrowserAccessError(
				'personal-access-token-required',
				'Browser access requires a configured Personal Access Token.',
			);
		}
		return verifier;
	}

	async #stopReady(): Promise<void> {
		const ready = this.#ready;
		if (ready === undefined) {
			this.#state = { phase: 'stopped' };
			return;
		}
		this.#ready = undefined;
		this.#state = { mode: ready.state.mode, phase: 'stopping' };
		const results = await Promise.allSettled([ready.gateway.dispose(), ready.server.dispose()]);
		this.#state = { phase: 'stopped' };
		const failures = results.flatMap((result) =>
			result.status === 'rejected' ? [result.reason] : [],
		);
		if (failures.length > 0) {
			throw new DesktopBrowserAccessError(
				'gateway-unavailable',
				'Orchbee Browser access cleanup did not converge.',
				{ cause: new AggregateError(failures) },
			);
		}
	}

	#requireReady(): ReadyBrowserAccess {
		const ready = this.#ready;
		if (
			ready === undefined ||
			ready.gateway.backgroundFailure !== undefined ||
			ready.server.backgroundFailure !== undefined
		) {
			throw new DesktopBrowserAccessError(
				'gateway-unavailable',
				'Orchbee Browser access is not ready.',
			);
		}
		return ready;
	}

	#enqueue<Value>(operation: () => Promise<Value>): Promise<Value> {
		const result = this.#operationTail.then(operation);
		this.#operationTail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}
}

function validateBrowserLinks(
	origins: readonly string[],
	links: readonly { readonly origin: string; readonly url: string }[],
): ReadonlyMap<string, string> {
	if (links.length !== origins.length) {
		throw new Error('The Gateway did not return one Browser link per renderer Origin.');
	}
	const result = new Map<string, string>();
	for (const link of links) {
		if (!origins.includes(link.origin) || result.has(link.origin)) {
			throw new Error('The Gateway returned an unexpected Browser Origin.');
		}
		const url = new URL(link.url);
		if (
			url.origin !== link.origin ||
			url.hash !== '' ||
			new URLSearchParams(url.search).getAll('orchbee-runtime').length !== 1 ||
			new URLSearchParams(url.search).get('orchbee-runtime') !== 'gateway' ||
			decodeGatewayBrowserSearch(url.search) === null
		) {
			throw new Error('The Gateway returned an invalid Browser access link.');
		}
		result.set(link.origin, link.url);
	}
	return result;
}

function invalidOriginError(): DesktopBrowserAccessError {
	return new DesktopBrowserAccessError(
		'invalid-origin',
		'The requested Browser access Origin is not active.',
	);
}

function mapNetworkError(error: unknown): DesktopBrowserAccessError {
	return new DesktopBrowserAccessError(
		'network-address-unavailable',
		'No supported local-network address is available for Browser access.',
		{ cause: error },
	);
}

function quiescingError(): DesktopBrowserAccessError {
	return new DesktopBrowserAccessError('desktop-quiescing', 'Desktop Browser access is quiescing.');
}

function personalAccessTokenStoreError(cause?: unknown): DesktopBrowserAccessError {
	return new DesktopBrowserAccessError(
		'personal-access-token-store-unavailable',
		'The Personal Access Token verifier store is unavailable.',
		cause === undefined ? undefined : { cause },
	);
}
