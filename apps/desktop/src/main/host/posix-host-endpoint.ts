import { type ChildProcess, spawn } from 'node:child_process';
import { Duplex, Readable, Writable } from 'node:stream';

import { HostConnection } from './HostConnection';

const AUTHENTICATED_STATUS_READY = 1;
const AUTHENTICATED_STATUS_ENDPOINT_UNAVAILABLE = 2;
const AUTHENTICATED_STATUS_IDENTITY_MISMATCH = 3;
const AUTHENTICATED_STATUS_IDENTITY_UNAVAILABLE = 4;
const AUTHENTICATED_STATUS_INSECURE_ENDPOINT = 5;
const AUTHENTICATED_STATUS_SYSTEM_ERROR = 6;
const retiredHelpers = new WeakSet<ChildProcess>();

export interface PosixHostEndpointDependencies {
	readonly createConnection: (transport: Duplex) => HostConnection;
	readonly spawn: typeof spawn;
}

const defaultDependencies: PosixHostEndpointDependencies = {
	createConnection: (transport) => new HostConnection(transport),
	spawn,
};

export function preparePosixHostRuntimeDirectory(
	binaryPath: string,
	runtimeDirectory: string,
	dependencies: PosixHostEndpointDependencies = defaultDependencies,
): Promise<void> {
	const child = dependencies.spawn(
		binaryPath,
		['posix-prepare-runtime', '--runtime-directory', runtimeDirectory],
		{ stdio: 'ignore' },
	);
	return waitForSuccessfulExit(child, 'POSIX Terminal Host runtime-directory preparation failed.');
}

export function connectAuthenticatedPosixHost(
	binaryPath: string,
	runtimeDirectory: string,
	endpointId: string,
	dependencies: PosixHostEndpointDependencies = defaultDependencies,
): Promise<HostConnection> {
	const child = dependencies.spawn(
		binaryPath,
		['posix-connect', '--runtime-directory', runtimeDirectory, '--endpoint-id', endpointId],
		{ stdio: ['pipe', 'pipe', 'ignore', 'pipe'] },
	);
	const input = child.stdin;
	const output = child.stdout;
	const status = child.stdio[3];
	if (input === null || output === null || !(status instanceof Readable)) {
		retireHelper(child);
		return Promise.reject(new Error('POSIX Terminal Host helper stdio is unavailable.'));
	}

	return new Promise((resolve, reject) => {
		let settled = false;
		const removePreauthenticationListeners = (): void => {
			child.off('error', handleError);
			child.off('exit', handleExit);
			status.off('data', handleStatus);
			status.off('end', handleStatusEnd);
		};
		const rejectBeforeAuthentication = (error: Error): void => {
			if (settled) return;
			settled = true;
			removePreauthenticationListeners();
			status.destroy();
			input.destroy();
			output.destroy();
			retireHelper(child);
			reject(error);
		};
		const handleError = (error: Error): void => rejectBeforeAuthentication(error);
		const handleExit = (code: number | null, signal: NodeJS.Signals | null): void => {
			rejectBeforeAuthentication(
				new Error(
					`POSIX Terminal Host helper exited before authentication (${formatExit(code, signal)}).`,
				),
			);
		};
		const handleStatusEnd = (): void => {
			rejectBeforeAuthentication(
				new Error('POSIX Terminal Host helper closed before reporting peer authentication.'),
			);
		};
		const handleStatus = (chunk: unknown): void => {
			if (!(chunk instanceof Uint8Array) || chunk.byteLength !== 1) {
				rejectBeforeAuthentication(
					new Error('POSIX Terminal Host helper emitted an invalid authentication status.'),
				);
				return;
			}
			const statusCode = chunk[0];
			if (statusCode !== AUTHENTICATED_STATUS_READY) {
				rejectBeforeAuthentication(authenticationFailure(statusCode));
				return;
			}

			settled = true;
			removePreauthenticationListeners();
			status.destroy();
			let transport: Duplex | undefined;
			try {
				transport = Duplex.fromWeb({
					readable: Readable.toWeb(output),
					writable: Writable.toWeb(input),
				});
				bindAuthenticatedHelperLifetime(child, transport);
				resolve(dependencies.createConnection(transport));
			} catch (error) {
				transport?.destroy();
				input.destroy();
				output.destroy();
				retireHelper(child);
				reject(
					error instanceof Error
						? error
						: new Error('POSIX Terminal Host connection construction failed.'),
				);
			}
		};

		child.once('error', handleError);
		child.once('exit', handleExit);
		status.once('data', handleStatus);
		status.once('end', handleStatusEnd);
	});
}

function waitForSuccessfulExit(child: ChildProcess, failureMessage: string): Promise<void> {
	return new Promise((resolve, reject) => {
		let settled = false;
		const handleError = (error: Error): void => {
			if (settled) return;
			settled = true;
			child.off('exit', handleExit);
			reject(error);
		};
		const handleExit = (code: number | null, signal: NodeJS.Signals | null): void => {
			if (settled) return;
			settled = true;
			child.off('error', handleError);
			if (code === 0 && signal === null) {
				resolve();
				return;
			}
			reject(new Error(`${failureMessage} ${formatExit(code, signal)}.`));
		};
		child.once('error', handleError);
		child.once('exit', handleExit);
	});
}

function bindAuthenticatedHelperLifetime(child: ChildProcess, transport: Duplex): void {
	const handleError = (error: Error): void => {
		transport.destroy(error);
	};
	const handleExit = (code: number | null, signal: NodeJS.Signals | null): void => {
		if (transport.destroyed) return;
		if (code === 0 && signal === null) {
			transport.destroy();
			return;
		}
		transport.destroy(
			new Error(`POSIX Terminal Host helper exited (${formatExit(code, signal)}).`),
		);
	};
	child.once('error', handleError);
	child.once('exit', handleExit);
	transport.once('close', () => {
		child.off('error', handleError);
		child.off('exit', handleExit);
		if (child.exitCode === null && child.signalCode === null) retireHelper(child);
	});
}

function retireHelper(child: ChildProcess): void {
	if (child.exitCode !== null || child.signalCode !== null) return;
	if (retiredHelpers.has(child)) return;
	retiredHelpers.add(child);
	child.once('error', ignoreRetiredHelperError);
	try {
		child.kill();
	} catch {
		// A concurrent helper exit already releases its stdio and authenticated socket.
	}
}

function ignoreRetiredHelperError(): void {
	// A retired helper can report a late spawn or termination error after ownership ended.
}

function authenticationFailure(status: number | undefined): Error {
	switch (status) {
		case AUTHENTICATED_STATUS_ENDPOINT_UNAVAILABLE:
			return new Error('POSIX Terminal Host endpoint is unavailable.');
		case AUTHENTICATED_STATUS_IDENTITY_MISMATCH:
			return new Error('POSIX Terminal Host peer effective UID does not match Desktop.');
		case AUTHENTICATED_STATUS_IDENTITY_UNAVAILABLE:
			return new Error('POSIX Terminal Host peer credentials are unavailable.');
		case AUTHENTICATED_STATUS_INSECURE_ENDPOINT:
			return new Error('POSIX Terminal Host runtime directory or endpoint is insecure.');
		case AUTHENTICATED_STATUS_SYSTEM_ERROR:
			return new Error('POSIX Terminal Host endpoint authentication failed.');
		default:
			return new Error('POSIX Terminal Host helper emitted an unknown authentication status.');
	}
}

function formatExit(code: number | null, signal: NodeJS.Signals | null): string {
	if (signal !== null) return `signal ${signal}`;
	return `status ${code ?? 'unknown'}`;
}
