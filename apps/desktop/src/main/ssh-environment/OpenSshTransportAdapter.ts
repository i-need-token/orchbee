import {
	spawn,
	type ChildProcessWithoutNullStreams,
	type SpawnOptionsWithStdioTuple,
} from 'node:child_process';
import { isAbsolute } from 'node:path';
import type { Readable, Writable } from 'node:stream';

import type {
	SshEnvironmentResolvedTarget,
	SshEnvironmentTargetReference,
} from './ssh-environment-contract';

const MAXIMUM_DIAGNOSTIC_BYTES = 64 * 1024;

export interface SshAskPassLaunchEnvironment {
	readonly environment: Readonly<NodeJS.ProcessEnv>;
}

export interface OpenSshTransport {
	readonly readable: Readable;
	readonly writable: Writable;
	diagnostics(): string;
	onExit(
		listener: (exit: {
			readonly code: null | number;
			readonly signal: null | NodeJS.Signals;
		}) => void,
	): void;
	retire(): void;
}

interface OpenSshTransportAdapterDependencies {
	readonly spawn: (
		executablePath: string,
		arguments_: readonly string[],
		options: SpawnOptionsWithStdioTuple<'pipe', 'pipe', 'pipe'>,
	) => ChildProcessWithoutNullStreams;
}

const defaultDependencies: OpenSshTransportAdapterDependencies = {
	spawn: (executablePath, arguments_, options) => spawn(executablePath, arguments_, options),
};

export class OpenSshTransportAdapter {
	readonly #dependencies: OpenSshTransportAdapterDependencies;
	readonly #executablePath: string;

	constructor(
		executablePath: string,
		dependencies: OpenSshTransportAdapterDependencies = defaultDependencies,
	) {
		if (!isAbsolute(executablePath)) {
			throw new Error('OpenSSH executable path must be absolute.');
		}
		this.#executablePath = executablePath;
		this.#dependencies = dependencies;
	}

	open(options: {
		readonly askPass: SshAskPassLaunchEnvironment;
		readonly remoteCommand: string;
		readonly resolvedTarget: SshEnvironmentResolvedTarget;
		readonly target: SshEnvironmentTargetReference;
	}): OpenSshTransport {
		const child: ChildProcessWithoutNullStreams = this.#dependencies.spawn(
			this.#executablePath,
			createOpenSshArguments(options),
			{
				env: { ...process.env, ...options.askPass.environment },
				stdio: ['pipe', 'pipe', 'pipe'],
				windowsHide: true,
			},
		);
		let diagnostics = '';
		child.stderr.setEncoding('utf8');
		child.stderr.on('data', (chunk: string) => {
			diagnostics = `${diagnostics}${chunk}`.slice(-MAXIMUM_DIAGNOSTIC_BYTES);
		});

		return {
			readable: child.stdout,
			writable: child.stdin,
			diagnostics: () => diagnostics,
			onExit: (listener) => {
				child.once('close', (code, signal) => listener({ code, signal }));
			},
			retire: () => {
				child.stdin.destroy();
				child.stdout.destroy();
				child.stderr.destroy();
				if (child.exitCode === null && child.signalCode === null) child.kill('SIGTERM');
			},
		};
	}
}

function createOpenSshArguments(options: {
	readonly remoteCommand: string;
	readonly resolvedTarget: SshEnvironmentResolvedTarget;
	readonly target: SshEnvironmentTargetReference;
}): string[] {
	const { resolvedTarget, target } = options;
	const arguments_ = [
		'-T',
		'-o',
		'AddKeysToAgent=no',
		'-o',
		'BatchMode=no',
		'-o',
		'ClearAllForwardings=yes',
		'-o',
		'ConnectionAttempts=1',
		'-o',
		'ConnectTimeout=15',
		'-o',
		'ControlMaster=no',
		'-o',
		'ControlPath=none',
		'-o',
		'ControlPersist=no',
		'-o',
		'EscapeChar=none',
		'-o',
		'ForwardAgent=no',
		'-o',
		'ForkAfterAuthentication=no',
		'-o',
		'ForwardX11=no',
		'-o',
		'ForwardX11Trusted=no',
		'-o',
		'GSSAPIDelegateCredentials=no',
		'-o',
		`HostKeyAlias=${resolvedTarget.hostKeyLookupName}`,
		'-o',
		`HostName=${resolvedTarget.hostname}`,
		'-o',
		'LogLevel=ERROR',
		'-o',
		'NumberOfPasswordPrompts=3',
		'-o',
		'PermitLocalCommand=no',
		'-o',
		`Port=${resolvedTarget.port}`,
		'-o',
		'RemoteCommand=none',
		'-o',
		'RequestTTY=no',
		'-o',
		'ServerAliveCountMax=3',
		'-o',
		'ServerAliveInterval=15',
		'-o',
		'SessionType=default',
		'-o',
		'StdinNull=no',
		'-o',
		`StrictHostKeyChecking=${resolvedTarget.hostKeyPolicy}`,
		'-o',
		'TCPKeepAlive=yes',
		'-o',
		'Tunnel=no',
		'-o',
		`User=${resolvedTarget.user}`,
	];
	if (target.configurationFilePath !== null) {
		arguments_.push('-F', target.configurationFilePath);
	}
	arguments_.push('--', target.destination, options.remoteCommand);
	return arguments_;
}
