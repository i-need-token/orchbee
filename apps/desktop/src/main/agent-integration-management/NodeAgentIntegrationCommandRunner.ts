import { spawn } from 'node:child_process';
import { constants as fileSystemConstants } from 'node:fs';
import { access, stat } from 'node:fs/promises';
import path from 'node:path';

import { resolveAgentExecutableCandidates } from './resolveAgentExecutableCandidates';

const COMMAND_OUTPUT_LIMIT_BYTES = 256 * 1024;
const COMMAND_TIMEOUT_MILLISECONDS = 3_000;
const SHELL_PATH_MARKER = '__ORCHBEE_AGENT_SEARCH_PATH_V1__';
const SHELL_PATH_PROBE_EXECUTABLE_ENVIRONMENT_KEY = 'ORCHBEE_AGENT_ENVIRONMENT_PROBE_EXECUTABLE';
const SHELL_PATH_PROBE_COMMAND = `"$${SHELL_PATH_PROBE_EXECUTABLE_ENVIRONMENT_KEY}" -p '"${SHELL_PATH_MARKER}" + JSON.stringify(process.env.PATH ?? null) + "${SHELL_PATH_MARKER}"'`;

export type AgentIntegrationCommandResult =
	| { readonly kind: 'completed'; readonly stdout: string }
	| { readonly kind: 'failed' | 'output-too-large' | 'timed-out' };

export interface AgentIntegrationCommandRunner {
	resolveExecutable(executableName: string): Promise<string | null>;
	run(
		executablePath: string,
		commandArguments: readonly string[],
	): Promise<AgentIntegrationCommandResult>;
}

export interface NodeAgentIntegrationCommandRunnerOptions {
	readonly environment: NodeJS.ProcessEnv;
	readonly homeDirectory: string;
	readonly shellExecutable: string;
}

export class NodeAgentIntegrationCommandRunner implements AgentIntegrationCommandRunner {
	readonly #environment: NodeJS.ProcessEnv;
	readonly #executableSearchPaths = new Map<string, string>();
	readonly #homeDirectory: string;
	readonly #shellExecutable: string;
	#shellSearchPathPromise: Promise<string | null> | undefined;

	constructor(options: NodeAgentIntegrationCommandRunnerOptions) {
		this.#environment = { ...options.environment, NO_COLOR: '1' };
		this.#homeDirectory = options.homeDirectory;
		this.#shellExecutable = options.shellExecutable;
	}

	async resolveExecutable(executableName: string): Promise<string | null> {
		const inheritedResolution = resolveAgentExecutableCandidates(executableName, [
			this.#environment['PATH'],
		]);
		if (inheritedResolution.kind === 'invalid-name') return null;
		const inheritedExecutable = await findFirstExecutable(inheritedResolution.paths);
		if (inheritedExecutable !== null) return inheritedExecutable;

		const shellSearchPath = await this.#resolveShellSearchPath();
		if (shellSearchPath === null) return null;
		const shellResolution = resolveAgentExecutableCandidates(executableName, [shellSearchPath]);
		if (shellResolution.kind === 'invalid-name') return null;
		const shellExecutable = await findFirstExecutable(shellResolution.paths);
		if (shellExecutable !== null) {
			this.#executableSearchPaths.set(shellExecutable, shellSearchPath);
		}
		return shellExecutable;
	}

	async run(
		executablePath: string,
		commandArguments: readonly string[],
	): Promise<AgentIntegrationCommandResult> {
		if (!path.isAbsolute(executablePath)) return { kind: 'failed' };
		const shellSearchPath = this.#executableSearchPaths.get(executablePath);
		const environment =
			shellSearchPath === undefined
				? this.#environment
				: { ...this.#environment, PATH: shellSearchPath };
		return runCommand(executablePath, commandArguments, environment, this.#homeDirectory);
	}

	#resolveShellSearchPath(): Promise<string | null> {
		this.#shellSearchPathPromise ??= this.#probeShellSearchPath();
		return this.#shellSearchPathPromise;
	}

	async #probeShellSearchPath(): Promise<string | null> {
		if (!path.isAbsolute(this.#shellExecutable)) return null;
		const result = await runCommand(
			this.#shellExecutable,
			createShellPathProbeArguments(this.#shellExecutable),
			{
				...this.#environment,
				ELECTRON_NO_ATTACH_CONSOLE: '1',
				ELECTRON_RUN_AS_NODE: '1',
				[SHELL_PATH_PROBE_EXECUTABLE_ENVIRONMENT_KEY]: process.execPath,
				ORCHBEE_RESOLVING_AGENT_ENVIRONMENT: '1',
			},
			this.#homeDirectory,
		);
		return result.kind === 'completed' ? parseShellSearchPath(result.stdout) : null;
	}
}

function createShellPathProbeArguments(shellExecutable: string): readonly string[] {
	const shellName = path.basename(shellExecutable).toLowerCase();
	return shellName === 'csh' || shellName === 'tcsh'
		? ['-ic', SHELL_PATH_PROBE_COMMAND]
		: ['-i', '-l', '-c', SHELL_PATH_PROBE_COMMAND];
}

async function findFirstExecutable(candidates: readonly string[]): Promise<string | null> {
	for (const candidate of candidates) {
		try {
			const [, metadata] = await Promise.all([
				access(candidate, fileSystemConstants.X_OK),
				stat(candidate),
			]);
			if (metadata.isFile()) return candidate;
		} catch {
			// An unavailable candidate is an ordinary search miss.
		}
	}
	return null;
}

function parseShellSearchPath(stdout: string): string | null {
	const endIndex = stdout.lastIndexOf(SHELL_PATH_MARKER);
	if (endIndex < 0) return null;
	const startIndex = stdout.lastIndexOf(SHELL_PATH_MARKER, endIndex - 1);
	if (startIndex < 0) return null;
	const encodedSearchPath = stdout.slice(startIndex + SHELL_PATH_MARKER.length, endIndex);
	try {
		const parsedSearchPath: unknown = JSON.parse(encodedSearchPath);
		return typeof parsedSearchPath === 'string' ? parsedSearchPath : null;
	} catch {
		return null;
	}
}

function runCommand(
	executablePath: string,
	commandArguments: readonly string[],
	environment: NodeJS.ProcessEnv,
	homeDirectory: string,
): Promise<AgentIntegrationCommandResult> {
	return new Promise((resolve) => {
		let settled = false;
		let outputBytes = 0;
		const output: Buffer[] = [];
		const child = spawn(executablePath, commandArguments, {
			cwd: homeDirectory,
			env: environment,
			stdio: ['ignore', 'pipe', 'ignore'],
		});
		const finish = (result: AgentIntegrationCommandResult): void => {
			if (settled) return;
			settled = true;
			clearTimeout(timer);
			resolve(result);
		};
		const timer = setTimeout(() => {
			child.kill('SIGTERM');
			finish({ kind: 'timed-out' });
		}, COMMAND_TIMEOUT_MILLISECONDS);
		child.stdout.on('data', (chunk: Buffer) => {
			if (settled) return;
			outputBytes += chunk.byteLength;
			if (outputBytes > COMMAND_OUTPUT_LIMIT_BYTES) {
				child.kill('SIGTERM');
				finish({ kind: 'output-too-large' });
				return;
			}
			output.push(chunk);
		});
		child.once('error', () => finish({ kind: 'failed' }));
		child.once('close', (exitCode) => {
			finish(
				exitCode === 0
					? { kind: 'completed', stdout: Buffer.concat(output).toString('utf8') }
					: { kind: 'failed' },
			);
		});
	});
}
