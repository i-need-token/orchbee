import { spawn } from 'node:child_process';

const COMMAND_OUTPUT_LIMIT_BYTES = 256 * 1024;
const COMMAND_TIMEOUT_MILLISECONDS = 3_000;
const EXECUTABLE_NAME_PATTERN = /^[a-z][a-z0-9-]{0,31}$/u;

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
	readonly #homeDirectory: string;
	readonly #shellExecutable: string;

	constructor(options: NodeAgentIntegrationCommandRunnerOptions) {
		this.#environment = { ...options.environment, NO_COLOR: '1' };
		this.#homeDirectory = options.homeDirectory;
		this.#shellExecutable = options.shellExecutable;
	}

	async resolveExecutable(executableName: string): Promise<string | null> {
		if (!EXECUTABLE_NAME_PATTERN.test(executableName)) return null;
		const result = await runCommand(
			this.#shellExecutable,
			['-lc', `command -v ${executableName}`],
			this.#environment,
			this.#homeDirectory,
		);
		if (result.kind !== 'completed') return null;
		const [firstLine] = result.stdout.trim().split(/\r?\n/u);
		return firstLine !== undefined && firstLine.startsWith('/') ? firstLine : null;
	}

	run(
		executablePath: string,
		commandArguments: readonly string[],
	): Promise<AgentIntegrationCommandResult> {
		if (!executablePath.startsWith('/')) return Promise.resolve({ kind: 'failed' });
		return runCommand(executablePath, commandArguments, this.#environment, this.#homeDirectory);
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
