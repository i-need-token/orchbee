import type { NetworkInterfaceInfo } from 'node:os';

import { describe, expect, it, vi } from 'vitest';

import type { LocalDevelopmentGatewayRuntimeOptions } from '../../gateway';
import { DesktopBrowserAccessRuntime } from '../DesktopBrowserAccessRuntime';

type RuntimeDependencies = ConstructorParameters<typeof DesktopBrowserAccessRuntime>[1];
type DesktopBrowserAccessGatewayHandle = ReturnType<
	NonNullable<RuntimeDependencies['createGateway']>
>;
type DesktopBrowserAccessServerHandle = ReturnType<
	NonNullable<RuntimeDependencies['createServer']>
>;

describe('DesktopBrowserAccessRuntime', () => {
	it('starts loopback once and confines open and copy effects to active secret links', async () => {
		const harness = createHarness({});
		const runtime = harness.runtime;

		const first = runtime.start('loopback');
		const second = runtime.start('loopback');
		const state = await first;
		await second;
		await runtime.openLocalBrowser();
		await runtime.copyAccessLink('http://127.0.0.1:43110');

		expect(state).toEqual({
			localOrigin: 'http://127.0.0.1:43110',
			mode: 'loopback',
			networkOrigins: [],
			phase: 'ready',
		});
		expect(JSON.stringify(runtime.getSnapshot())).not.toContain('secret-capability');
		expect(harness.createServer).toHaveBeenCalledOnce();
		expect(harness.createGateway).toHaveBeenCalledOnce();
		expect(harness.openExternal).toHaveBeenCalledWith(
			'http://127.0.0.1:43110/?orchbee-runtime=gateway#secret-capability',
		);
		expect(harness.writeClipboardText).toHaveBeenCalledWith(
			'http://127.0.0.1:43110/?orchbee-runtime=gateway#secret-capability',
		);
		await expect(runtime.copyAccessLink('http://127.0.0.1:9999')).rejects.toMatchObject({
			code: 'invalid-origin',
		});
	});

	it('replaces loopback with explicit LAN listeners and publishes only private origins', async () => {
		const lifecycle: string[] = [];
		const harness = createHarness(
			{
				en0: [address('192.168.50.12')],
				en1: [address('10.1.2.3')],
				public: [address('203.0.113.10')],
			},
			lifecycle,
		);

		await harness.runtime.start('loopback');
		const state = await harness.runtime.start('local-network');

		expect(state).toEqual({
			localOrigin: 'http://127.0.0.1:43110',
			mode: 'local-network',
			networkOrigins: ['http://10.1.2.3:43110', 'http://192.168.50.12:43110'],
			phase: 'ready',
		});
		expect(harness.createServer.mock.calls[1]?.[0]).toMatchObject({ bindHost: '0.0.0.0' });
		expect(harness.createGateway.mock.calls[1]?.[0]).toEqual({
			bindHost: '0.0.0.0',
			entryPath: '/resources/orchbee-gateway.cjs',
			rendererOrigins: [
				'http://127.0.0.1:43110',
				'http://10.1.2.3:43110',
				'http://192.168.50.12:43110',
			],
		});
		expect(lifecycle.indexOf('gateway:dispose:0')).toBeLessThan(
			lifecycle.indexOf('server:create:1'),
		);
		expect(lifecycle).not.toContain('http://203.0.113.10:43110');
	});

	it('cleans a partial start, reports the owning stage, and permits retry', async () => {
		let gatewayAttempt = 0;
		const harness = createHarness({}, [], (options) => {
			const gateway = createGateway([], options.rendererOrigins, gatewayAttempt);
			if (gatewayAttempt === 0) gateway.start.mockRejectedValueOnce(new Error('listen failed'));
			gatewayAttempt += 1;
			return gateway;
		});

		await expect(harness.runtime.start('loopback')).rejects.toMatchObject({
			code: 'gateway-unavailable',
		});
		expect(harness.servers[0]?.dispose).toHaveBeenCalledOnce();
		expect(harness.gateways[0]?.dispose).toHaveBeenCalledOnce();

		await expect(harness.runtime.start('loopback')).resolves.toMatchObject({ phase: 'ready' });
		expect(harness.createGateway).toHaveBeenCalledTimes(2);
	});

	it('fails LAN preview before creating a listener when no private address exists', async () => {
		const harness = createHarness({ en0: [address('203.0.113.10')] });

		await expect(harness.runtime.start('local-network')).rejects.toEqual(
			expect.objectContaining({ code: 'network-address-unavailable' }),
		);
		expect(harness.createServer).not.toHaveBeenCalled();
	});

	it('revokes a start that is still waiting when Desktop disposal begins', async () => {
		const startDeferred = createDeferred<{ readonly port: number }>();
		const server = createServer([], 0);
		server.start.mockReturnValueOnce(startDeferred.promise);
		const harness = createHarness({}, [], undefined, () => server);
		const starting = harness.runtime.start('loopback');
		await vi.waitFor(() => expect(harness.createServer).toHaveBeenCalledOnce());
		const disposal = harness.runtime.dispose();

		startDeferred.resolve({ port: 43_110 });

		await expect(starting).rejects.toMatchObject({ code: 'desktop-quiescing' });
		await disposal;
		expect(server.dispose).toHaveBeenCalledOnce();
		expect(harness.createGateway).not.toHaveBeenCalled();
		expect(harness.runtime.getSnapshot()).toEqual({ phase: 'stopped' });
	});
});

function createHarness(
	interfaces: Readonly<Record<string, readonly NetworkInterfaceInfo[] | undefined>>,
	lifecycle: string[] = [],
	createGatewayOverride?: (
		options: LocalDevelopmentGatewayRuntimeOptions,
	) => DesktopBrowserAccessGatewayHandle,
	createServerOverride?: () => DesktopBrowserAccessServerHandle,
): {
	readonly createGateway: ReturnType<typeof vi.fn>;
	readonly createServer: ReturnType<typeof vi.fn>;
	readonly gateways: ReturnType<typeof createGateway>[];
	readonly openExternal: ReturnType<typeof vi.fn>;
	readonly runtime: DesktopBrowserAccessRuntime;
	readonly servers: ReturnType<typeof createServer>[];
	readonly writeClipboardText: ReturnType<typeof vi.fn>;
} {
	const gateways: ReturnType<typeof createGateway>[] = [];
	const servers: ReturnType<typeof createServer>[] = [];
	const createServerMock = vi.fn<
		(options: { readonly bindHost: string }) => DesktopBrowserAccessServerHandle
	>((options) => {
		const index = servers.length;
		lifecycle.push(`server:create:${index}`);
		const server =
			(createServerOverride?.() as ReturnType<typeof createServer> | undefined) ??
			createServer(lifecycle, index);
		servers.push(server);
		expect(options.bindHost).toMatch(/^(?:127\.0\.0\.1|0\.0\.0\.0)$/u);
		return server;
	});
	const createGatewayMock = vi.fn<
		(options: LocalDevelopmentGatewayRuntimeOptions) => DesktopBrowserAccessGatewayHandle
	>((options) => {
		const index = gateways.length;
		lifecycle.push(`gateway:create:${index}`);
		const gateway =
			(createGatewayOverride?.(options) as ReturnType<typeof createGateway> | undefined) ??
			createGateway(lifecycle, options.rendererOrigins, index);
		gateways.push(gateway);
		return gateway;
	});
	const openExternal = vi.fn<(url: string) => Promise<void>>(async () => undefined);
	const writeClipboardText = vi.fn<(text: string) => void>();
	const runtime = new DesktopBrowserAccessRuntime(
		{
			gatewayEntryPath: '/resources/orchbee-gateway.cjs',
			rendererRoot: '/resources/dist',
			serverEntryPath: '/resources/orchbee-server.cjs',
		},
		{
			createGateway: createGatewayMock,
			createServer: createServerMock,
			getNetworkInterfaces: () => interfaces,
			openClient: async () => {
				throw new Error('No Browser page was opened by this lifecycle test.');
			},
			openExternal,
			writeClipboardText,
		},
	);
	return {
		createGateway: createGatewayMock,
		createServer: createServerMock,
		gateways,
		openExternal,
		runtime,
		servers,
		writeClipboardText,
	};
}

function createServer(lifecycle: string[], index: number) {
	return {
		dispose: vi.fn<DesktopBrowserAccessServerHandle['dispose']>(async () => {
			lifecycle.push(`server:dispose:${index}`);
		}),
		start: vi.fn<DesktopBrowserAccessServerHandle['start']>(async () => ({ port: 43_110 })),
	} satisfies DesktopBrowserAccessServerHandle;
}

function createGateway(lifecycle: string[], origins: readonly string[], index: number) {
	return {
		dispose: vi.fn<DesktopBrowserAccessGatewayHandle['dispose']>(async () => {
			lifecycle.push(`gateway:dispose:${index}`);
		}),
		start: vi.fn<DesktopBrowserAccessGatewayHandle['start']>(async () => ({
			browserUrl: browserUrl(origins[0] ?? ''),
			browserUrls: origins.map((origin) => ({ origin, url: browserUrl(origin) })),
		})),
	} satisfies DesktopBrowserAccessGatewayHandle;
}

function browserUrl(origin: string): string {
	return `${origin}/?orchbee-runtime=gateway#secret-capability`;
}

function address(value: string): NetworkInterfaceInfo {
	return {
		address: value,
		cidr: null,
		family: 'IPv4',
		internal: false,
		mac: '00:00:00:00:00:00',
		netmask: '255.255.255.0',
		scopeid: 0,
	};
}

function createDeferred<Value>(): {
	readonly promise: Promise<Value>;
	readonly resolve: (value: Value) => void;
} {
	let resolvePromise: (value: Value) => void = () => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}
