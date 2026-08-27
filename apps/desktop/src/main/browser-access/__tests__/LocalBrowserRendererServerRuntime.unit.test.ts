import {
	BrowserSessionSigningKeySchema,
	PersonalAccessTokenVerifierSchema,
} from '@orchbee/remote-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import { LocalBrowserRendererServerRuntime } from '../LocalBrowserRendererServerRuntime';

type RuntimeDependencies = NonNullable<
	ConstructorParameters<typeof LocalBrowserRendererServerRuntime>[1]
>;
type DesktopServerUtilityProcess = ReturnType<NonNullable<RuntimeDependencies['createProcess']>>;
const authentication = {
	personalAccessTokenVerifier: PersonalAccessTokenVerifierSchema.parse('v'.repeat(43)),
	sessionSigningKey: BrowserSessionSigningKeySchema.parse('s'.repeat(43)),
};

describe('LocalBrowserRendererServerRuntime', () => {
	it('accepts one exact readiness record and coalesces start', async () => {
		const process = new FakeServerProcess();
		const runtime = new LocalBrowserRendererServerRuntime(
			{
				authentication,
				bindHost: '0.0.0.0',
				entryPath: '/resources/orchbee-server.cjs',
				rendererRoot: '/resources/dist',
			},
			{
				createProcess: () => process,
				scheduleStartTimeout: () => ({}) as ReturnType<typeof setTimeout>,
			},
		);

		const first = runtime.start();
		const second = runtime.start();
		process.emitLine('ordinary child diagnostic');
		process.emitLine('ORCHBEE_SERVER_LISTENING={"host":"0.0.0.0","port":43110}');

		await expect(first).resolves.toEqual({ port: 43_110 });
		await expect(second).resolves.toEqual({ port: 43_110 });
		await runtime.dispose();
		expect(process.kill).toHaveBeenCalledOnce();
	});

	it('rejects mismatched readiness and kills the partial process', async () => {
		const process = new FakeServerProcess();
		const runtime = new LocalBrowserRendererServerRuntime(
			{
				authentication,
				bindHost: '127.0.0.1',
				entryPath: '/resources/orchbee-server.cjs',
				rendererRoot: '/resources/dist',
			},
			{
				createProcess: () => process,
				scheduleStartTimeout: () => ({}) as ReturnType<typeof setTimeout>,
			},
		);
		const starting = runtime.start();

		process.emitLine('ORCHBEE_SERVER_LISTENING={"host":"0.0.0.0","port":43110}');

		await expect(starting).rejects.toThrow(/invalid readiness/u);
		await vi.waitFor(() => expect(process.kill).toHaveBeenCalledOnce());
	});

	it('rejects startup when the process exits before readiness', async () => {
		const process = new FakeServerProcess();
		const runtime = new LocalBrowserRendererServerRuntime(
			{
				authentication,
				bindHost: '127.0.0.1',
				entryPath: '/resources/orchbee-server.cjs',
				rendererRoot: '/resources/dist',
			},
			{
				createProcess: () => process,
				scheduleStartTimeout: () => ({}) as ReturnType<typeof setTimeout>,
			},
		);
		const starting = runtime.start();

		process.emitExit(1);

		await expect(starting).rejects.toThrow(/exited unexpectedly/u);
	});
});

class FakeServerProcess implements DesktopServerUtilityProcess {
	readonly #exitListeners = new Set<(code: number) => void>();
	readonly kill = vi.fn<() => void>();
	readonly #lineListeners = new Set<(line: string) => void>();

	emitExit(code: number): void {
		for (const listener of this.#exitListeners) listener(code);
	}

	emitLine(line: string): void {
		for (const listener of this.#lineListeners) listener(line);
	}

	onExit(listener: (code: number) => void): () => void {
		this.#exitListeners.add(listener);
		return () => this.#exitListeners.delete(listener);
	}

	onStdoutLine(listener: (line: string) => void): () => void {
		this.#lineListeners.add(listener);
		return () => this.#lineListeners.delete(listener);
	}
}
