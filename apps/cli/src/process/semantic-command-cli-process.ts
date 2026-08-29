import { randomUUID } from 'node:crypto';
import { open } from 'node:fs/promises';
import path from 'node:path';
import type { Readable } from 'node:stream';

import { SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES } from '@orchbee/semantic-command-protocol/local-http';

import { runSemanticCommandCli, type SemanticCommandCliRunResult } from '../application';
import { connectLocalSemanticCommandCoordinator } from '../local-transport';

export interface SemanticCommandCliProcessEnvironment {
	readonly ORCHBEE_CLI_NATIVE_ADDON_PATH?: string | undefined;
	readonly ORCHBEE_CLI_RUNTIME_DIRECTORY?: string | undefined;
}

export interface SemanticCommandCliProcessOptions {
	readonly arguments: readonly string[];
	readonly environment: SemanticCommandCliProcessEnvironment;
	readonly signal?: AbortSignal | undefined;
	readonly stdin: Readable;
}

export interface SemanticCommandCliProcessHost {
	readonly arguments: readonly string[];
	readonly environment: SemanticCommandCliProcessEnvironment;
	readonly stdin: Readable;
	registerAbort(abort: () => void): () => void;
	setExitCode(exitCode: SemanticCommandCliRunResult['exitCode']): void;
	writeStderr(value: string): void;
	writeStdout(value: string): void;
}

interface SemanticCommandCliLocalConfiguration {
	readonly addonPath: string;
	readonly runtimeDirectory: string;
}

export async function runSemanticCommandCliProcess(
	options: SemanticCommandCliProcessOptions,
): Promise<SemanticCommandCliRunResult> {
	const configuration = resolveLocalConfiguration(options.environment);
	return runSemanticCommandCli(options.arguments, {
		connect: async (endpointId) => {
			if (configuration === null) {
				return { code: 'coordinator-unavailable', kind: 'error' };
			}
			const connection = await connectLocalSemanticCommandCoordinator({
				addonPath: configuration.addonPath,
				...(endpointId === undefined ? {} : { endpointId }),
				runtimeDirectory: configuration.runtimeDirectory,
			});
			return connection.kind === 'connected'
				? {
						client: connection.client,
						coordinatorGeneration: connection.locator.coordinatorGeneration,
						kind: 'connected',
					}
				: connection;
		},
		createRequestIdentity: () => `cli:request:${randomUUID()}`,
		readInput: (source) => readBoundedInput(source, options.stdin),
		...(options.signal === undefined ? {} : { signal: options.signal }),
	});
}

export async function startSemanticCommandCliProcess(
	host: SemanticCommandCliProcessHost = defaultProcessHost(),
): Promise<void> {
	const abortController = new AbortController();
	const removeAbortHandlers = host.registerAbort(abortController.abort.bind(abortController));
	try {
		const result = await runSemanticCommandCliProcess({
			arguments: host.arguments,
			environment: host.environment,
			signal: abortController.signal,
			stdin: host.stdin,
		});
		host.writeStdout(result.stdout);
		if (result.stderr.length > 0) host.writeStderr(result.stderr);
		host.setExitCode(result.exitCode);
	} finally {
		removeAbortHandlers();
	}
}

function resolveLocalConfiguration(
	environment: SemanticCommandCliProcessEnvironment,
): SemanticCommandCliLocalConfiguration | null {
	const addonPath = environment.ORCHBEE_CLI_NATIVE_ADDON_PATH;
	const runtimeDirectory = environment.ORCHBEE_CLI_RUNTIME_DIRECTORY;
	if (
		addonPath === undefined ||
		runtimeDirectory === undefined ||
		!path.isAbsolute(addonPath) ||
		!path.isAbsolute(runtimeDirectory)
	) {
		return null;
	}
	return {
		addonPath: path.normalize(addonPath),
		runtimeDirectory: path.normalize(runtimeDirectory),
	};
}

async function readBoundedInput(source: string, stdin: Readable): Promise<Uint8Array> {
	return source === '-' ? readBoundedStream(stdin) : readBoundedFile(source);
}

async function readBoundedFile(source: string): Promise<Uint8Array> {
	const file = await open(source, 'r');
	try {
		const buffer = Buffer.alloc(SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES + 1);
		let offset = 0;
		while (offset < buffer.byteLength) {
			const result = await file.read(buffer, offset, buffer.byteLength - offset, null);
			if (result.bytesRead === 0) break;
			offset += result.bytesRead;
		}
		return buffer.subarray(0, offset);
	} finally {
		await file.close();
	}
}

async function readBoundedStream(stdin: Readable): Promise<Uint8Array> {
	const buffer = Buffer.alloc(SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES + 1);
	let offset = 0;
	for await (const candidate of stdin) {
		const chunk = readableChunk(candidate);
		const remaining = buffer.byteLength - offset;
		if (chunk.byteLength >= remaining) {
			chunk.copy(buffer, offset, 0, remaining);
			return buffer;
		}
		chunk.copy(buffer, offset);
		offset += chunk.byteLength;
	}
	return buffer.subarray(0, offset);
}

function readableChunk(candidate: unknown): Buffer {
	if (typeof candidate === 'string') return Buffer.from(candidate, 'utf8');
	if (candidate instanceof Uint8Array) return Buffer.from(candidate);
	throw new Error('CLI stdin produced a non-byte chunk.');
}

function defaultProcessHost(): SemanticCommandCliProcessHost {
	return {
		arguments: process.argv.slice(2),
		environment: process.env,
		registerAbort(abort) {
			process.once('SIGINT', abort);
			process.once('SIGTERM', abort);
			return () => {
				process.off('SIGINT', abort);
				process.off('SIGTERM', abort);
			};
		},
		setExitCode(exitCode) {
			process.exitCode = exitCode;
		},
		stdin: process.stdin,
		writeStderr(value) {
			process.stderr.write(value);
		},
		writeStdout(value) {
			process.stdout.write(value);
		},
	};
}
