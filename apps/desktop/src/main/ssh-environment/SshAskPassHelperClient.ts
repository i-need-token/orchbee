import { connect } from 'node:net';

const MAXIMUM_RESPONSE_BYTES = 32 * 1024;
const RESPONSE_TIMEOUT_MILLISECONDS = 5 * 60 * 1_000;
const OPENSSH_HOST_CONFIRMATION_SUFFIX =
	/Are you sure you want to continue connecting \(yes\/no(?:\/\[fingerprint\])?\)\?\s*$/u;

export interface SshAskPassHelperClientOptions {
	readonly arguments?: readonly string[];
	readonly environment?: Readonly<NodeJS.ProcessEnv>;
	readonly writeOutput?: (output: string) => void;
}

export class SshAskPassHelperClient {
	readonly #arguments: readonly string[];
	readonly #environment: Readonly<NodeJS.ProcessEnv>;
	readonly #writeOutput: (output: string) => void;

	constructor(options: SshAskPassHelperClientOptions = {}) {
		this.#arguments = options.arguments ?? process.argv.slice(2);
		this.#environment = options.environment ?? process.env;
		this.#writeOutput = options.writeOutput ?? ((output) => process.stdout.write(output));
	}

	async run(): Promise<0 | 1> {
		try {
			const socketPath = this.#requiredEnvironment('ORCHBEE_ASKPASS_SOCKET');
			const capability = this.#requiredEnvironment('ORCHBEE_ASKPASS_CAPABILITY');
			const prompt = this.#arguments[0] ?? '';
			const promptKind = this.#environment['SSH_ASKPASS_PROMPT'];
			const kind = classifyPromptKind(promptKind, prompt);
			const response = await this.#request(socketPath, {
				version: 1,
				capability,
				kind,
				prompt,
			});
			if (!this.#isSubmittedResponse(response)) return 1;
			this.#writeOutput(`${response.response}\n`);
			return 0;
		} catch {
			return 1;
		}
	}

	#request(socketPath: string, payload: unknown): Promise<unknown> {
		return new Promise((resolve, reject) => {
			const socket = connect(socketPath);
			let bytes = Buffer.alloc(0);
			let ended = false;
			socket.setTimeout(RESPONSE_TIMEOUT_MILLISECONDS, () => {
				socket.destroy(new Error('AskPass response timed out.'));
			});
			socket.once('connect', () => socket.write(`${JSON.stringify(payload)}\n`));
			socket.on('data', (chunk: Buffer) => {
				bytes = Buffer.concat([bytes, chunk]);
				if (bytes.byteLength > MAXIMUM_RESPONSE_BYTES) {
					socket.destroy(new Error('AskPass response exceeded its bound.'));
				}
			});
			socket.once('end', () => {
				ended = true;
				try {
					resolve(JSON.parse(bytes.toString('utf8')));
				} catch (error: unknown) {
					reject(error);
				}
			});
			socket.once('error', reject);
			socket.once('close', () => {
				if (!ended) reject(new Error('AskPass response closed before completion.'));
			});
		});
	}

	#requiredEnvironment(name: string): string {
		const value = this.#environment[name];
		if (value === undefined || value.length === 0) {
			throw new Error('AskPass environment is invalid.');
		}
		return value;
	}

	#isSubmittedResponse(value: unknown): value is { readonly response: string } {
		if (typeof value !== 'object' || value === null) return false;
		if (!('kind' in value) || !('response' in value)) return false;
		return (
			value.kind === 'submitted' &&
			typeof value.response === 'string' &&
			this.#hasNoResponseControlCharacters(value.response)
		);
	}

	#hasNoResponseControlCharacters(response: string): boolean {
		for (const character of response) {
			if (character === '\u0000' || character === '\r' || character === '\n') return false;
		}
		return true;
	}
}

function classifyPromptKind(
	promptKind: string | undefined,
	prompt: string,
): 'confirm' | 'none' | 'unknown' {
	if (promptKind === 'confirm' || promptKind === 'none') return promptKind;
	return OPENSSH_HOST_CONFIRMATION_SUFFIX.test(prompt) ? 'confirm' : 'unknown';
}
