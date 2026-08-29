import { randomBytes } from 'node:crypto';
import { chmod, mkdtemp, rm, stat, writeFile } from 'node:fs/promises';
import { createServer, type Server, type Socket } from 'node:net';
import { tmpdir } from 'node:os';
import { isAbsolute, join } from 'node:path';

import { z } from 'zod';

import type { SshAskPassLaunchEnvironment } from './OpenSshTransportAdapter';

const ASK_PASS_PROTOCOL_VERSION = 1;
const MAXIMUM_ASK_PASS_REQUEST_BYTES = 32 * 1024;
const MAXIMUM_ASK_PASS_PROMPT_CHARACTERS = 4096;
const MAXIMUM_ASK_PASS_RESPONSE_CHARACTERS = 4096;
const MAXIMUM_PROMPTS_PER_GENERATION = 8;
const MAXIMUM_OPEN_ASK_PASS_SOCKETS = 8;
const ASK_PASS_REQUEST_TIMEOUT_MILLISECONDS = 5_000;

const AskPassRequestSchema = z.strictObject({
	version: z.literal(ASK_PASS_PROTOCOL_VERSION),
	capability: z
		.string()
		.length(64)
		.regex(/^[0-9a-f]+$/u),
	kind: z.enum(['confirm', 'none', 'unknown']),
	prompt: z
		.string()
		.max(MAXIMUM_ASK_PASS_PROMPT_CHARACTERS)
		.refine((prompt) => !prompt.includes('\0')),
});

const AskPassResponseSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('cancelled') }),
	z.strictObject({
		kind: z.literal('submitted'),
		response: z
			.string()
			.max(MAXIMUM_ASK_PASS_RESPONSE_CHARACTERS)
			.refine(hasNoResponseControlCharacters),
	}),
]);

export interface SshAskPassInteractionRequest {
	readonly handle: string;
	readonly kind: 'confirm' | 'none' | 'unknown';
	readonly prompt: string;
}

function hasNoResponseControlCharacters(response: string): boolean {
	for (const character of response) {
		if (character === '\u0000' || character === '\r' || character === '\n') return false;
	}
	return true;
}

export type SshAskPassInteractionResult = z.infer<typeof AskPassResponseSchema>;

export interface SshAskPassBridgeOptions {
	readonly helperEntryPath: string;
	readonly interaction: (
		request: SshAskPassInteractionRequest,
	) => Promise<SshAskPassInteractionResult>;
	readonly runtimeExecutablePath: string;
	readonly temporaryRoot?: string;
}

export class SshAskPassBridge {
	readonly #options: SshAskPassBridgeOptions;
	readonly #sockets = new Set<Socket>();
	#activeRequest = false;
	#capability: string | undefined;
	#cleanupPromise: Promise<void> | undefined;
	#directory: string | undefined;
	#disposeRequested = false;
	#promptCount = 0;
	#server: Server | undefined;
	#startPromise: Promise<SshAskPassLaunchEnvironment> | undefined;

	constructor(options: SshAskPassBridgeOptions) {
		if (!isAbsolute(options.helperEntryPath) || !isAbsolute(options.runtimeExecutablePath)) {
			throw new Error('AskPass helper and runtime paths must be absolute.');
		}
		this.#options = options;
	}

	async start(): Promise<SshAskPassLaunchEnvironment> {
		if (this.#startPromise !== undefined || this.#disposeRequested) {
			throw new Error('AskPass bridge cannot be started again.');
		}
		const startPromise = this.#start();
		this.#startPromise = startPromise;
		return startPromise;
	}

	async #start(): Promise<SshAskPassLaunchEnvironment> {
		if (process.platform === 'win32') throw new Error('AskPass bridge is unsupported on Windows.');

		const temporaryRoot =
			this.#options.temporaryRoot ?? (process.platform === 'darwin' ? '/tmp' : tmpdir());
		const directory = await mkdtemp(join(temporaryRoot, 'orchbee-askpass-'));
		this.#directory = directory;
		try {
			await chmod(directory, 0o700);
			const launcherPath = join(directory, 'askpass');
			const socketPath = join(directory, 'a.s');
			await writeFile(
				launcherPath,
				'#!/bin/sh\nexec "$ORCHBEE_ASKPASS_RUNTIME" "$ORCHBEE_ASKPASS_ENTRY" "$@"\n',
				{ encoding: 'utf8', mode: 0o700 },
			);
			const capability = randomBytes(32).toString('hex');
			this.#capability = capability;
			const server = createServer((socket) => this.#accept(socket));
			this.#server = server;
			await new Promise<void>((resolve, reject) => {
				server.once('error', reject);
				server.listen(socketPath, () => {
					server.off('error', reject);
					resolve();
				});
			});
			await chmod(socketPath, 0o600);
			const [directoryStatus, socketStatus] = await Promise.all([
				stat(directory),
				stat(socketPath),
			]);
			const effectiveUserId = process.geteuid?.();
			if (
				(directoryStatus.mode & 0o777) !== 0o700 ||
				(socketStatus.mode & 0o777) !== 0o600 ||
				(effectiveUserId !== undefined &&
					(directoryStatus.uid !== effectiveUserId || socketStatus.uid !== effectiveUserId))
			) {
				throw new Error('AskPass private IPC permissions are invalid.');
			}
			if (this.#disposeRequested) throw new Error('AskPass bridge was disposed during startup.');

			return {
				environment: {
					DISPLAY: 'orchbee-askpass',
					ELECTRON_RUN_AS_NODE: '1',
					ORCHBEE_ASKPASS_CAPABILITY: capability,
					ORCHBEE_ASKPASS_ENTRY: this.#options.helperEntryPath,
					ORCHBEE_ASKPASS_RUNTIME: this.#options.runtimeExecutablePath,
					ORCHBEE_ASKPASS_SOCKET: socketPath,
					SSH_ASKPASS: launcherPath,
					SSH_ASKPASS_REQUIRE: 'force',
				},
			};
		} catch (error: unknown) {
			await this.#cleanup();
			throw error;
		}
	}

	async dispose(): Promise<void> {
		this.#disposeRequested = true;
		try {
			await this.#startPromise;
		} catch {
			// Startup owns no durable result after failure; cleanup remains idempotent.
		}
		await this.#cleanup();
	}

	#cleanup(): Promise<void> {
		if (this.#cleanupPromise !== undefined) return this.#cleanupPromise;
		const cleanupPromise = this.#cleanupOwnedResources();
		this.#cleanupPromise = cleanupPromise;
		return cleanupPromise;
	}

	async #cleanupOwnedResources(): Promise<void> {
		for (const socket of this.#sockets) socket.destroy();
		this.#sockets.clear();
		const server = this.#server;
		this.#server = undefined;
		if (server?.listening === true) {
			await new Promise<void>((resolve) => server.close(() => resolve()));
		}
		const directory = this.#directory;
		this.#directory = undefined;
		this.#capability = undefined;
		this.#activeRequest = false;
		if (directory !== undefined) await rm(directory, { force: true, recursive: true });
	}

	#accept(socket: Socket): void {
		if (this.#sockets.size >= MAXIMUM_OPEN_ASK_PASS_SOCKETS) {
			socket.destroy();
			return;
		}
		this.#sockets.add(socket);
		socket.setTimeout(ASK_PASS_REQUEST_TIMEOUT_MILLISECONDS, () => socket.destroy());
		let requestBytes = Buffer.alloc(0);
		let handled = false;
		socket.on('data', (chunk: Buffer) => {
			if (handled) return;
			requestBytes = Buffer.concat([requestBytes, chunk]);
			if (requestBytes.byteLength > MAXIMUM_ASK_PASS_REQUEST_BYTES) {
				handled = true;
				socket.destroy();
				return;
			}
			const newline = requestBytes.indexOf(0x0a);
			if (newline < 0) return;
			handled = true;
			if (newline !== requestBytes.byteLength - 1) {
				socket.destroy();
				return;
			}
			socket.setTimeout(0);
			void this.#handle(socket, requestBytes.subarray(0, newline));
		});
		socket.once('close', () => this.#sockets.delete(socket));
		socket.once('error', () => socket.destroy());
	}

	async #handle(socket: Socket, bytes: Buffer): Promise<void> {
		let candidate: unknown;
		try {
			candidate = JSON.parse(bytes.toString('utf8'));
		} catch {
			socket.destroy();
			return;
		}
		const parsed = AskPassRequestSchema.safeParse(candidate);
		if (
			!parsed.success ||
			parsed.data.capability !== this.#capability ||
			this.#activeRequest ||
			this.#promptCount >= MAXIMUM_PROMPTS_PER_GENERATION
		) {
			socket.destroy();
			return;
		}

		this.#activeRequest = true;
		this.#promptCount += 1;
		try {
			const result = AskPassResponseSchema.parse(
				await this.#options.interaction({
					handle: `ssh-prompt-${randomBytes(16).toString('hex')}`,
					kind: parsed.data.kind,
					prompt: parsed.data.prompt,
				}),
			);
			socket.end(`${JSON.stringify(result)}\n`);
		} catch {
			socket.end('{"kind":"cancelled"}\n');
		} finally {
			this.#activeRequest = false;
		}
	}
}
