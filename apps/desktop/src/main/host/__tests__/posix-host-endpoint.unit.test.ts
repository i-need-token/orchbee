import type { ChildProcess, spawn } from 'node:child_process';
import { EventEmitter, once } from 'node:events';
import { PassThrough } from 'node:stream';
import type { Duplex } from 'node:stream';

import {
	CorrelationIdSchema,
	CURRENT_PROTOCOL_VERSION,
	encodeProtocolEnvelope,
	HostEpochSchema,
	MessageKind,
	PayloadEncoding,
	PROTOCOL_LIMITS,
} from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import type { HostConnection } from '../HostConnection';
import {
	connectAuthenticatedPosixHost,
	preparePosixHostRuntimeDirectory,
	type PosixHostEndpointDependencies,
} from '../posix-host-endpoint';

interface ControlledHelper {
	readonly child: ChildProcess;
	readonly input: PassThrough;
	readonly kill: ReturnType<typeof vi.fn<() => boolean>>;
	readonly output: PassThrough;
	readonly status: PassThrough;
}

describe('POSIX Host endpoint helper', () => {
	it('closes a credential-mismatched helper before HostConnection or Hello can exist', async () => {
		const helper = createControlledHelper();
		const helloBytes: Uint8Array[] = [];
		helper.input.on('data', (chunk: Uint8Array) => helloBytes.push(chunk));
		helper.output.write(createHostileHelloAccepted());
		const createConnection = vi.fn<(transport: Duplex) => HostConnection>();
		const dependencies = createDependencies(helper, createConnection);
		let connection: HostConnection | undefined;
		let hostEpoch: string | undefined;
		let protocolVersion: object | undefined;
		const pending = connectAuthenticatedPosixHost(
			'/opt/orchbee/orchbee-terminal-host',
			'/private/runtime',
			'ab'.repeat(16),
			dependencies,
		).then((value) => {
			connection = value;
			hostEpoch = value.hostEpoch;
			protocolVersion = value.protocolVersion;
			return value;
		});

		helper.status.write(Uint8Array.of(3));
		await expect(pending).rejects.toThrow('peer effective UID does not match');
		expect(createConnection).not.toHaveBeenCalled();
		expect(connection).toBeUndefined();
		expect(hostEpoch).toBeUndefined();
		expect(protocolVersion).toBeUndefined();
		expect(helloBytes).toHaveLength(0);
		expect(helper.output.destroyed).toBe(true);
		expect(helper.kill).toHaveBeenCalledOnce();

		helper.child.emit('error', new Error('late helper error'));
		helper.child.emit('exit', 1, null);
		expect(helper.kill).toHaveBeenCalledOnce();
	});

	it('fails closed when kernel peer credentials are unavailable', async () => {
		const helper = createControlledHelper();
		const createConnection = vi.fn<(transport: Duplex) => HostConnection>();
		const pending = connectAuthenticatedPosixHost(
			'/opt/orchbee/orchbee-terminal-host',
			'/private/runtime',
			'bc'.repeat(16),
			createDependencies(helper, createConnection),
		);

		helper.status.write(Uint8Array.of(4));
		await expect(pending).rejects.toThrow('peer credentials are unavailable');
		expect(createConnection).not.toHaveBeenCalled();
		expect(helper.kill).toHaveBeenCalledOnce();
	});

	it('constructs the protocol connection only after native same-UID authentication', async () => {
		const helper = createControlledHelper();
		const expected = {} as HostConnection;
		let transport: Duplex | undefined;
		const createConnection = vi.fn<(value: Duplex) => HostConnection>((value) => {
			transport = value;
			return expected;
		});
		const dependencies = createDependencies(helper, createConnection);
		const pending = connectAuthenticatedPosixHost(
			'/opt/orchbee/orchbee-terminal-host',
			'/private/runtime',
			'cd'.repeat(16),
			dependencies,
		);

		expect(dependencies.spawn).toHaveBeenCalledWith(
			'/opt/orchbee/orchbee-terminal-host',
			['connect', '--runtime-directory', '/private/runtime', '--endpoint-id', 'cd'.repeat(16)],
			{ stdio: ['pipe', 'pipe', 'ignore', 'pipe'] },
		);
		expect(createConnection).not.toHaveBeenCalled();
		helper.status.write(Uint8Array.of(1));
		await expect(pending).resolves.toBe(expected);
		expect(createConnection).toHaveBeenCalledOnce();
		if (transport === undefined) throw new Error('Authenticated transport was not constructed.');
		const closed = once(transport, 'close');
		transport.destroy();
		await closed;
		expect(helper.kill).toHaveBeenCalledOnce();
	});

	it('releases the authenticated helper when protocol-connection construction fails', async () => {
		const helper = createControlledHelper();
		const pending = connectAuthenticatedPosixHost(
			'/opt/orchbee/orchbee-terminal-host',
			'/private/runtime',
			'de'.repeat(16),
			createDependencies(helper, () => {
				throw new Error('injected construction failure');
			}),
		);

		helper.status.write(Uint8Array.of(1));
		await expect(pending).rejects.toThrow('injected construction failure');
		expect(helper.input.destroyed).toBe(true);
		expect(helper.output.destroyed).toBe(true);
		expect(helper.kill).toHaveBeenCalledOnce();
	});

	it('requires successful native runtime-directory preparation', async () => {
		const successful = createControlledHelper();
		const successfulDependencies = createDependencies(
			successful,
			vi.fn<(transport: Duplex) => HostConnection>(),
		);
		const successfulPreparation = preparePosixHostRuntimeDirectory(
			'/opt/orchbee/orchbee-terminal-host',
			'/private/runtime',
			successfulDependencies,
		);
		expect(successfulDependencies.spawn).toHaveBeenCalledWith(
			'/opt/orchbee/orchbee-terminal-host',
			['prepare-runtime', '--runtime-directory', '/private/runtime'],
			{ stdio: 'ignore' },
		);
		successful.child.emit('exit', 0, null);
		await expect(successfulPreparation).resolves.toBeUndefined();

		const failed = createControlledHelper();
		const failedPreparation = preparePosixHostRuntimeDirectory(
			'/opt/orchbee/orchbee-terminal-host',
			'/private/runtime',
			createDependencies(failed, vi.fn<(transport: Duplex) => HostConnection>()),
		);
		failed.child.emit('exit', 1, null);
		await expect(failedPreparation).rejects.toThrow('status 1');
	});
});

function createDependencies(
	helper: ControlledHelper,
	createConnection: (transport: Duplex) => HostConnection,
): PosixHostEndpointDependencies {
	const spawnHelper = vi.fn<() => ChildProcess>(() => helper.child) as unknown as typeof spawn;
	return { createConnection, spawn: spawnHelper };
}

function createControlledHelper(): ControlledHelper {
	const emitter = new EventEmitter();
	const input = new PassThrough();
	const output = new PassThrough();
	const status = new PassThrough();
	const kill = vi.fn<() => boolean>(() => true);
	const child = Object.assign(emitter, {
		exitCode: null,
		kill,
		signalCode: null,
		stderr: null,
		stdin: input,
		stdio: [input, output, null, status],
		stdout: output,
	}) as unknown as ChildProcess;
	return { child, input, kill, output, status };
}

function createHostileHelloAccepted(): Uint8Array {
	const hostEpoch = HostEpochSchema.parse('aa'.repeat(16));
	const response = encodeProtocolEnvelope({
		correlationId: CorrelationIdSchema.parse('bb'.repeat(16)),
		event: false,
		hostEpoch,
		kind: MessageKind.helloAccepted,
		optionalMessage: false,
		payload: new TextEncoder().encode(
			JSON.stringify({
				frameBytes: PROTOCOL_LIMITS.maxFrameBytes,
				hostEpoch,
				hostVersion: '0.1.0',
				inputBytes: PROTOCOL_LIMITS.maxInputBytes,
				protocol: CURRENT_PROTOCOL_VERSION,
				rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
				supportedOptionalFeatures: [],
			}),
		),
		payloadEncoding: PayloadEncoding.controlJson,
		protocol: CURRENT_PROTOCOL_VERSION,
		response: true,
	});
	if (!response.ok) throw new Error(response.detail);
	return response.value;
}
