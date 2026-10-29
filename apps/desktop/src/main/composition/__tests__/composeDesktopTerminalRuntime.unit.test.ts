import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	MessageKind,
	TerminalIdSchema,
	TerminalIdentitySchema,
	type TerminalExited,
	type TerminalIdentity,
} from '@orchbee/terminal-protocol';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '@orchbee/workbench-client-protocol';

import type { HostConnection } from '../../host';
import {
	composeDesktopTerminalRuntime,
	type DesktopTerminalRuntimeComposition,
} from '../composeDesktopTerminalRuntime';
import { resolveDesktopRuntimeConfiguration } from '../resolve-desktop-runtime-configuration';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories.splice(0).map(async (directory) => rm(directory, { recursive: true })),
	);
});

describe('composeDesktopTerminalRuntime', () => {
	it('starts with no eager terminal or projection when the binding ledger is absent', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-runtime-'));
		temporaryDirectories.push(directory);
		const createBroker = vi.fn<DesktopTerminalRuntimeComposition['createBroker']>();
		const createMessageChannel = vi.fn<DesktopTerminalRuntimeComposition['createMessageChannel']>();
		const host = {
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl: vi.fn<HostConnection['requestControl']>(),
			requestTransportEnded: false,
		} as unknown as HostConnection;
		const configuration = resolveDesktopRuntimeConfiguration({
			systemShellExecutable:
				process.platform === 'win32' ? 'C:\\Windows\\System32\\cmd.exe' : '/bin/sh',
			applicationPath: directory,
			environment: { ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: directory },
			homeDirectory: directory,
			platform: process.platform,
			posixRuntimeBaseDirectory: '/private/var/folders/user/T',
			terminalHostName: 'orchbee-host',
			userDataPath: directory,
		});
		const runtime = composeDesktopTerminalRuntime({
			configuration,
			createBroker,
			createMessageChannel,
			host,
			randomIdentifier: () => '11'.repeat(16),
			resolveTerminalLaunch: () => configuration.terminalLaunch,
		});

		await expect(runtime.initialize()).resolves.toEqual({ kind: 'ready' });
		expect(runtime.getRecoveredPresentations()).toEqual([]);
		expect(runtime.getProjectionMetrics()).toEqual([]);
		expect(createBroker).not.toHaveBeenCalled();
		expect(createMessageChannel).not.toHaveBeenCalled();
		await runtime.dispose();
	});

	it('keeps the Main publication handoff after delayed identity association', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-runtime-'));
		temporaryDirectories.push(directory);
		const hostEpoch = HostEpochSchema.parse('22'.repeat(16));
		const identity: TerminalIdentity = {
			creationId: CreationIdSchema.parse('11'.repeat(16)),
			hostEpoch,
			terminalId: TerminalIdSchema.parse('33'.repeat(16)),
		};
		const createRequested = createDeferred<void>();
		const createResponse = createDeferred<TerminalIdentity>();
		const requestControl = vi.fn<HostConnection['requestControl']>(async (kind) => {
			if (kind === MessageKind.listTerminals) {
				return {
					inventoryGeneration: '1',
					nextCursor: null,
					terminals: [],
				} as never;
			}
			if (kind === MessageKind.createTerminal) {
				createRequested.resolve(undefined);
				return createResponse.promise as never;
			}
			throw new Error(`Unexpected Host request ${kind}.`);
		});
		const host = {
			hostEpoch,
			onClosed: vi.fn<HostConnection['onClosed']>(() => vi.fn<() => void>()),
			requestControl,
			requestTransportEnded: false,
		} as unknown as HostConnection;
		const configuration = resolveDesktopRuntimeConfiguration({
			systemShellExecutable:
				process.platform === 'win32' ? 'C:\\Windows\\System32\\cmd.exe' : '/bin/sh',
			applicationPath: directory,
			environment: { ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: directory },
			homeDirectory: directory,
			platform: process.platform,
			posixRuntimeBaseDirectory: '/private/var/folders/user/T',
			terminalHostName: 'orchbee-host',
			userDataPath: directory,
		});
		const runtime = composeDesktopTerminalRuntime({
			configuration,
			createBroker: vi.fn<DesktopTerminalRuntimeComposition['createBroker']>(),
			createMessageChannel: vi.fn<DesktopTerminalRuntimeComposition['createMessageChannel']>(),
			host,
			randomIdentifier: () => '11'.repeat(16),
			resolveTerminalLaunch: () => configuration.terminalLaunch,
		});

		await expect(runtime.initialize()).resolves.toEqual({ kind: 'ready' });
		const presentationId = terminalPresentationId('presentation-delayed-exit');
		const provisioning = runtime.execute({
			launchCwd: directory,
			operationId: terminalPresentationOperationId('provision-delayed-exit'),
			presentationId,
			type: 'terminal-provision-requested',
		});
		await expect(createRequested.promise).resolves.toBeUndefined();
		expect(requestControl).toHaveBeenCalledWith(
			MessageKind.createTerminal,
			expect.anything(),
			expect.anything(),
		);

		const event: TerminalExited = {
			exit: { code: 17, kind: 'exit-code' },
			finalRevision: CanonicalUint64TextSchema.parse('2'),
			identity,
		};
		await expect(runtime.recordExitOutcome(event)).resolves.toMatchObject({
			kind: 'retryable',
			reason: 'association',
		});
		createResponse.resolve(TerminalIdentitySchema.parse(identity));
		await expect(provisioning).resolves.toMatchObject({
			presentationId,
			type: 'terminal-provision-completed',
		});

		await expect(runtime.recordExitOutcome(event)).resolves.toEqual({
			finalRevision: event.finalRevision,
			kind: 'durable',
			presentationId,
		});
		await expect(runtime.recordExitOutcome(event)).resolves.toEqual({
			finalRevision: event.finalRevision,
			kind: 'durable',
			presentationId,
		});
		expect(runtime.commitExitPublication?.(event)).toBe(true);
		await expect(runtime.recordExitOutcome(event)).resolves.toEqual({
			finalRevision: event.finalRevision,
			kind: 'committed',
			presentationId,
		});
		expect(runtime.getDurableTerminalExit(presentationId)).toEqual(event);
		await runtime.dispose();
	});
});

function createDeferred<Value>(): { promise: Promise<Value>; resolve: (value: Value) => void } {
	let resolvePromise: (value: Value) => void = () => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}
