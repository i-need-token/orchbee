import { workbenchSharedDocumentRevision } from '@orchbee/workbench/document';
import { describe, expect, it } from 'vitest';

import type { DesktopDevelopmentRestartRequest } from '../desktop-development-contract';
import type {
	DesktopDevelopmentRestartPlan,
	DesktopDevelopmentRestartPlanLoadResult,
} from '../DesktopDevelopmentRestartPlanStore';
import {
	DesktopDevelopmentRuntime,
	type DesktopDevelopmentHostBuild,
	type DesktopDevelopmentRuntimeDependencies,
	type DesktopDevelopmentSupervisedProcess,
} from '../DesktopDevelopmentRuntime';

class FakeProcess implements DesktopDevelopmentSupervisedProcess {
	connected = true;
	exited = false;
	readonly requests: DesktopDevelopmentRestartRequest[] = [];
	readonly #onSend: (request: DesktopDevelopmentRestartRequest) => void;
	readonly #trace: string[];
	readonly #name: string;

	constructor(
		name: string,
		trace: string[],
		onSend: (request: DesktopDevelopmentRestartRequest) => void = () => undefined,
	) {
		this.#name = name;
		this.#onSend = onSend;
		this.#trace = trace;
	}

	async send(message: DesktopDevelopmentRestartRequest): Promise<boolean> {
		this.requests.push(message);
		this.#trace.push(`send:${this.#name}:${message.reason}`);
		this.#onSend(message);
		return true;
	}

	async terminate(): Promise<void> {
		this.#trace.push(`terminate:${this.#name}`);
		this.exited = true;
		this.connected = false;
	}

	async waitForExit(): Promise<boolean> {
		this.#trace.push(`wait:${this.#name}`);
		return this.exited;
	}
}

function createHarness(
	prepareHostRestart: boolean,
	options: { readonly failInitialHostSpawn?: boolean } = {},
) {
	const trace: string[] = [];
	let hostBuildListener: ((build: DesktopDevelopmentHostBuild) => Promise<void>) | undefined;
	let plan: DesktopDevelopmentRestartPlan | undefined;
	let electronSequence = 0;
	const hostProcesses: FakeProcess[] = [];
	const electronProcesses: FakeProcess[] = [];
	const dependencies: DesktopDevelopmentRuntimeDependencies = {
		buildInitialHost: async () => {
			trace.push('build-host');
			return {
				binaryPath: '/repo/apps/terminal-host/zig-out/bin/orchbee-terminal-host',
				digest: 'a',
				inputDigest: 'input-a',
			};
		},
		captureHostCandidate: async (build, _workspace, generation) => {
			trace.push(`capture-host:${generation}:${build.digest}`);
			return {
				binaryPath: `/runtime/host-${generation}`,
				digest: build.digest,
				generation,
				inputDigest: build.inputDigest,
			};
		},
		cleanupWorkspace: async () => {
			trace.push('cleanup-workspace');
		},
		createRestartPlan: () => ({
			load: async (): Promise<DesktopDevelopmentRestartPlanLoadResult> =>
				plan === undefined ? { kind: 'missing' } : { kind: 'available', plan },
			remove: async () => {
				trace.push('remove-plan');
				plan = undefined;
			},
			writePending: async (generation) => {
				trace.push(`pending-plan:${generation}`);
				plan = { generation, state: 'pending', version: 1 };
			},
		}),
		createWorkspace: async () => ({
			endpointId: 'ab'.repeat(16),
			recoveryDirectory: '/runtime/recovery',
			restartPlanPath: '/runtime/restart-plan.json',
			runtimeDirectory: '/runtime/host',
			terminalScrollbackBytes: '10000000',
		}),
		releaseHostCandidate: async (candidate) => {
			trace.push(`release:host-${candidate.generation}`);
		},
		spawnElectron: async () => {
			electronSequence += 1;
			const name = `electron-${electronSequence}`;
			trace.push(`spawn:${name}`);
			const process = new FakeProcess(name, trace, (request) => {
				if (request.reason === 'desktop') {
					process.exited = true;
					process.connected = false;
					return;
				}
				if (prepareHostRestart && plan?.state === 'pending') {
					plan = {
						documentRevision: workbenchSharedDocumentRevision('9'),
						generation: plan.generation,
						state: 'ready',
						version: 1,
					};
					process.exited = true;
					process.connected = false;
				}
			});
			electronProcesses.push(process);
			return process;
		},
		spawnHost: async ({ candidate }) => {
			if (options.failInitialHostSpawn === true && hostProcesses.length === 0) {
				trace.push(`fail:host-${candidate.generation}`);
				throw new Error('initial Host failed');
			}
			const process = new FakeProcess(`host-${candidate.generation}`, trace);
			trace.push(`spawn:host-${candidate.generation}`);
			hostProcesses.push(process);
			return process;
		},
		watchHostBuilds: async (_root, _digest, listener) => {
			trace.push('watch-host');
			hostBuildListener = listener;
			return {
				close: async () => {
					trace.push('close-watch');
				},
			};
		},
	};
	const runtime = new DesktopDevelopmentRuntime(
		{
			desktopEntryPath: '/repo/apps/desktop/dist/main/main.cjs',
			electronExecutable: '/repo/node_modules/electron/electron',
			environment: {},

			rendererUrl: 'http://127.0.0.1:3000/',
			restartTimeoutMilliseconds: 1,

			sourceRoot: '/repo',
			terminalHostRoot: '/repo/apps/terminal-host',
		},
		dependencies,
		{ error: () => undefined, info: () => undefined, warn: () => undefined },
	);
	return {
		electronProcesses,
		hostProcesses,
		runtime,
		async publishHostBuild(digest: string) {
			if (hostBuildListener === undefined) throw new Error('Host build watch is not ready.');
			await hostBuildListener({
				binaryPath: '/repo/apps/terminal-host/zig-out/bin/orchbee-terminal-host',
				digest,
				inputDigest: `input-${digest}`,
			});
		},
		trace,
	};
}

describe('DesktopDevelopmentRuntime', () => {
	it('closes the build watch and workspace when startup fails after acquiring them', async () => {
		const harness = createHarness(false, { failInitialHostSpawn: true });

		await expect(harness.runtime.start()).rejects.toThrow('initial Host failed');
		expect(harness.trace).toEqual(
			expect.arrayContaining([
				'watch-host',
				'fail:host-1',
				'close-watch',
				'release:host-1',
				'cleanup-workspace',
			]),
		);
		expect(harness.trace.indexOf('close-watch')).toBeLessThan(
			harness.trace.indexOf('cleanup-workspace'),
		);
	});

	it('restarts Electron in place and replaces Host only after a ready plan', async () => {
		const harness = createHarness(true);
		await harness.runtime.start();
		expect(harness.trace.indexOf('watch-host')).toBeLessThan(harness.trace.indexOf('spawn:host-1'));

		harness.runtime.requestDesktopRestart();
		await harness.runtime.waitForIdle();
		expect(harness.electronProcesses).toHaveLength(2);
		expect(harness.hostProcesses).toHaveLength(1);
		expect(harness.trace).toContain('send:electron-1:desktop');

		await harness.publishHostBuild('b');
		await harness.runtime.waitForIdle();
		expect(harness.trace).toEqual(
			expect.arrayContaining([
				'capture-host:2:b',
				'pending-plan:2',
				'send:electron-2:terminal-host',
				'spawn:host-2',
				'spawn:electron-3',
			]),
		);
		expect(harness.trace.indexOf('pending-plan:2')).toBeLessThan(
			harness.trace.indexOf('spawn:host-2'),
		);
		expect(harness.hostProcesses).toHaveLength(2);
		expect(harness.electronProcesses).toHaveLength(3);
		expect(harness.trace).toContain('release:host-1');

		await harness.runtime.stop();
		expect(harness.trace).toContain('release:host-2');
	});

	it('keeps the old Host and Electron when restart preparation is not ready', async () => {
		const harness = createHarness(false);
		await harness.runtime.start();
		await harness.publishHostBuild('b');
		await harness.runtime.waitForIdle();

		expect(harness.hostProcesses).toHaveLength(1);
		expect(harness.electronProcesses).toHaveLength(1);
		expect(harness.hostProcesses[0]?.exited).toBe(false);
		expect(harness.electronProcesses[0]?.exited).toBe(false);
		expect(harness.trace).not.toContain('spawn:host-2');
		expect(harness.trace).toContain('release:host-2');

		await harness.runtime.stop();
	});

	it('still applies a coalesced Desktop build when Host restart preparation aborts', async () => {
		const harness = createHarness(false);
		await harness.runtime.start();
		await harness.publishHostBuild('b');
		harness.runtime.requestDesktopRestart();
		await harness.runtime.waitForIdle();

		expect(harness.hostProcesses).toHaveLength(1);
		expect(harness.electronProcesses).toHaveLength(2);
		expect(harness.trace).toContain('send:electron-1:terminal-host');
		expect(harness.trace).toContain('send:electron-1:desktop');
		expect(harness.trace).toContain('spawn:electron-2');

		await harness.runtime.stop();
	});
});
