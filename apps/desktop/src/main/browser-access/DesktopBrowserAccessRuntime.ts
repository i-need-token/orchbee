import { networkInterfaces, type NetworkInterfaceInfo } from 'node:os';

import {
	type DesktopBrowserAccessFailureCode,
	type DesktopBrowserAccessMode,
	type DesktopBrowserAccessState,
} from '@orchbee/desktop-client-protocol';
import { clipboard, shell } from 'electron';

import {
	LocalDevelopmentGatewayRuntime,
	type DesktopGatewayClientHandle,
	type LocalDevelopmentGatewayRuntimeOptions,
} from '../gateway';
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
	readonly rendererRoot: string;
	readonly serverEntryPath: string;
}

interface DesktopBrowserAccessRuntimeDependencies {
	readonly createGateway: (
		options: LocalDevelopmentGatewayRuntimeOptions,
		openClient: () => Promise<DesktopGatewayClientHandle>,
	) => DesktopBrowserAccessGatewayHandle;
	readonly createServer: (options: {
		readonly bindHost: '0.0.0.0' | '127.0.0.1';
		readonly entryPath: string;
		readonly rendererRoot: string;
	}) => DesktopBrowserAccessServerHandle;
	readonly getNetworkInterfaces: () => Readonly<
		Record<string, readonly NetworkInterfaceInfo[] | undefined>
	>;
	readonly openClient: () => Promise<DesktopGatewayClientHandle>;
	readonly openExternal: (url: string) => Promise<void>;
	readonly writeClipboardText: (text: string) => void;
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
	#ready: ReadyBrowserAccess | undefined;
	#state: DesktopBrowserAccessState = { phase: 'stopped' };

	constructor(
		options: DesktopBrowserAccessRuntimeOptions,
		dependencies: Partial<DesktopBrowserAccessRuntimeDependencies> &
			Pick<DesktopBrowserAccessRuntimeDependencies, 'openClient'>,
	) {
		this.#options = options;
		this.#dependencies = {
			createGateway: (gatewayOptions, openClient) =>
				new LocalDevelopmentGatewayRuntime(gatewayOptions, { openClient }),
			createServer: (serverOptions) => new LocalBrowserRendererServerRuntime(serverOptions),
			getNetworkInterfaces: networkInterfaces,
			openExternal: (url) => shell.openExternal(url),
			writeClipboardText: (text) => clipboard.writeText(text),
			...dependencies,
		};
	}

	getSnapshot(): DesktopBrowserAccessState {
		return this.#state;
	}

	start(mode: DesktopBrowserAccessMode): Promise<DesktopBrowserAccessState> {
		if (this.#disposeRequested) return Promise.reject(quiescingError());
		return this.#enqueue(() => this.#start(mode));
	}

	stop(): Promise<DesktopBrowserAccessState> {
		if (this.#disposeRequested) return Promise.reject(quiescingError());
		return this.#enqueue(async () => {
			await this.#stopReady();
			return this.#state;
		});
	}

	openLocalBrowser(): Promise<DesktopBrowserAccessState> {
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
			return this.#state;
		});
	}

	copyAccessLink(origin: string): Promise<DesktopBrowserAccessState> {
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
			return this.#state;
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
			server = this.#dependencies.createServer({
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
		if (url.origin !== link.origin || url.hash.length <= 1) {
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
