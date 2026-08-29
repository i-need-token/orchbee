import { execFile } from 'node:child_process';
import { existsSync } from 'node:fs';
import { isAbsolute, win32 } from 'node:path';

import {
	SshEnvironmentResolvedTargetSchema,
	type SshEnvironmentHostKeyPolicy,
	type SshEnvironmentResolvedTarget,
	type SshEnvironmentTargetReference,
} from './ssh-environment-contract';

const OPEN_SSH_QUERY_TIMEOUT_MILLISECONDS = 5_000;
const OPEN_SSH_QUERY_OUTPUT_LIMIT_BYTES = 256 * 1_024;
const REQUIRED_CONFIGURATION_KEYS = new Set([
	'hostname',
	'hostkeyalias',
	'port',
	'stricthostkeychecking',
	'user',
]);

type OpenSshConfigurationQueryResult =
	| { readonly kind: 'completed'; readonly stdout: string }
	| {
			readonly kind: 'failed';
			readonly reason: 'executable-unavailable' | 'output-limit' | 'query-failed' | 'timed-out';
	  };

interface OpenSshTargetResolverDependencies {
	readonly fileExists?: ((path: string) => boolean) | undefined;
	readonly isAbsolute: (path: string) => boolean;
	readonly query: (
		executablePath: string,
		arguments_: readonly string[],
	) => Promise<OpenSshConfigurationQueryResult>;
}

type OpenSshTargetResolutionFailureCode =
	| 'configuration-path-not-absolute'
	| 'host-key-checking-disabled'
	| 'openssh-output-invalid'
	| 'openssh-output-too-large'
	| 'openssh-path-not-absolute'
	| 'openssh-query-failed'
	| 'openssh-query-timed-out'
	| 'openssh-unavailable';

export type OpenSshTargetResolutionResult =
	| { readonly kind: 'resolved'; readonly target: SshEnvironmentResolvedTarget }
	| { readonly code: OpenSshTargetResolutionFailureCode; readonly kind: 'failed' };

const defaultDependencies: OpenSshTargetResolverDependencies = {
	fileExists: existsSync,
	isAbsolute,
	query: queryOpenSshConfiguration,
};

export class OpenSshTargetResolver {
	readonly #defaultConfigurationFilePath: null | string;
	readonly #dependencies: OpenSshTargetResolverDependencies;
	readonly #executablePath: string;

	static resolveSystemExecutablePath(
		platform: NodeJS.Platform,
		windowsSystemRoot: string | undefined,
	): string {
		if (platform !== 'win32') return '/usr/bin/ssh';
		return windowsSystemRoot === undefined || windowsSystemRoot.length === 0
			? 'ssh'
			: win32.join(windowsSystemRoot, 'System32', 'OpenSSH', 'ssh.exe');
	}

	constructor(
		executablePath: string,
		dependencies: OpenSshTargetResolverDependencies = defaultDependencies,
		defaultConfigurationFilePath: null | string = null,
	) {
		this.#defaultConfigurationFilePath = defaultConfigurationFilePath;
		this.#executablePath = executablePath;
		this.#dependencies = dependencies;
	}

	async resolve(target: SshEnvironmentTargetReference): Promise<OpenSshTargetResolutionResult> {
		if (!this.#dependencies.isAbsolute(this.#executablePath)) {
			return { code: 'openssh-path-not-absolute', kind: 'failed' };
		}
		const defaultConfigurationFilePath = this.#defaultConfigurationFilePath;
		if (
			defaultConfigurationFilePath !== null &&
			!this.#dependencies.isAbsolute(defaultConfigurationFilePath)
		) {
			return { code: 'configuration-path-not-absolute', kind: 'failed' };
		}
		const configurationFilePath =
			target.configurationFilePath ??
			(defaultConfigurationFilePath !== null &&
			(this.#dependencies.fileExists ?? defaultDependencies.fileExists)?.(
				defaultConfigurationFilePath,
			)
				? defaultConfigurationFilePath
				: null);
		if (configurationFilePath !== null && !this.#dependencies.isAbsolute(configurationFilePath)) {
			return { code: 'configuration-path-not-absolute', kind: 'failed' };
		}
		const arguments_ = ['-G'];
		if (configurationFilePath !== null) {
			arguments_.push('-F', configurationFilePath);
		}
		arguments_.push('--', target.destination);
		const queryResult = await this.#dependencies.query(this.#executablePath, arguments_);
		if (queryResult.kind === 'failed') return mapQueryFailure(queryResult.reason);
		if (Buffer.byteLength(queryResult.stdout, 'utf8') > OPEN_SSH_QUERY_OUTPUT_LIMIT_BYTES) {
			return { code: 'openssh-output-too-large', kind: 'failed' };
		}
		return parseConfigurationOutput(target, queryResult.stdout);
	}
}

function queryOpenSshConfiguration(
	executablePath: string,
	arguments_: readonly string[],
): Promise<OpenSshConfigurationQueryResult> {
	return new Promise((resolve) => {
		execFile(
			executablePath,
			Array.from(arguments_),
			{
				encoding: 'utf8',
				maxBuffer: OPEN_SSH_QUERY_OUTPUT_LIMIT_BYTES,
				timeout: OPEN_SSH_QUERY_TIMEOUT_MILLISECONDS,
				windowsHide: true,
			},
			(error, stdout) => {
				if (error === null) {
					resolve({ kind: 'completed', stdout });
					return;
				}
				if (error.code === 'ENOENT') {
					resolve({ kind: 'failed', reason: 'executable-unavailable' });
				} else if (error.code === 'ERR_CHILD_PROCESS_STDIO_MAXBUFFER') {
					resolve({ kind: 'failed', reason: 'output-limit' });
				} else if (error.killed) {
					resolve({ kind: 'failed', reason: 'timed-out' });
				} else {
					resolve({ kind: 'failed', reason: 'query-failed' });
				}
			},
		);
	});
}

function mapQueryFailure(
	reason: Extract<OpenSshConfigurationQueryResult, { readonly kind: 'failed' }>['reason'],
): OpenSshTargetResolutionResult {
	switch (reason) {
		case 'executable-unavailable':
			return { code: 'openssh-unavailable', kind: 'failed' };
		case 'output-limit':
			return { code: 'openssh-output-too-large', kind: 'failed' };
		case 'query-failed':
			return { code: 'openssh-query-failed', kind: 'failed' };
		case 'timed-out':
			return { code: 'openssh-query-timed-out', kind: 'failed' };
	}
}

function parseConfigurationOutput(
	target: SshEnvironmentTargetReference,
	stdout: string,
): OpenSshTargetResolutionResult {
	const values = new Map<string, string>();
	for (const line of stdout.split(/\r?\n/u)) {
		const separator = line.indexOf(' ');
		if (separator <= 0) continue;
		const key = line.slice(0, separator).toLowerCase();
		if (!REQUIRED_CONFIGURATION_KEYS.has(key)) continue;
		if (values.has(key)) return { code: 'openssh-output-invalid', kind: 'failed' };
		values.set(key, line.slice(separator + 1));
	}
	const hostname = values.get('hostname');
	const portText = values.get('port');
	const strictHostKeyChecking = values.get('stricthostkeychecking');
	const user = values.get('user');
	if (
		hostname === undefined ||
		portText === undefined ||
		strictHostKeyChecking === undefined ||
		user === undefined
	) {
		return { code: 'openssh-output-invalid', kind: 'failed' };
	}
	const hostKeyPolicy = parseHostKeyPolicy(strictHostKeyChecking);
	if (hostKeyPolicy === 'disabled') {
		return { code: 'host-key-checking-disabled', kind: 'failed' };
	}
	if (hostKeyPolicy === 'invalid') {
		return { code: 'openssh-output-invalid', kind: 'failed' };
	}
	const parsed = SshEnvironmentResolvedTargetSchema.safeParse({
		destination: target.destination,
		hostKeyLookupName: values.get('hostkeyalias') ?? hostname,
		hostKeyPolicy,
		hostname,
		port: Number(portText),
		user,
	});
	return parsed.success
		? { kind: 'resolved', target: parsed.data }
		: { code: 'openssh-output-invalid', kind: 'failed' };
}

function parseHostKeyPolicy(value: string): SshEnvironmentHostKeyPolicy | 'disabled' | 'invalid' {
	switch (value) {
		case 'true':
		case 'yes':
			return 'yes';
		case 'ask':
		case 'accept-new':
			return value;
		case 'false':
		case 'no':
		case 'off':
			return 'disabled';
		default:
			return 'invalid';
	}
}
