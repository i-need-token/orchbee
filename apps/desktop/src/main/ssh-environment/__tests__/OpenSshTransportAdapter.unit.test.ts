import { spawn } from 'node:child_process';

import { describe, expect, it } from 'vitest';

import { OpenSshTransportAdapter } from '../OpenSshTransportAdapter';
import {
	SshEnvironmentResolvedTargetSchema,
	SshEnvironmentTargetReferenceSchema,
} from '../ssh-environment-contract';

describe('OpenSSH transport arguments', () => {
	it('rejects PATH-based executable lookup at construction', () => {
		expect(() => new OpenSshTransportAdapter('ssh')).toThrow(
			'OpenSSH executable path must be absolute.',
		);
	});

	it('uses an available application-home configuration for a target without an override', async () => {
		let arguments_: readonly string[] | undefined;
		const adapter = new OpenSshTransportAdapter(
			'/usr/bin/ssh',
			{
				fileExists: () => true,
				spawn: (_executablePath, receivedArguments, options) => {
					arguments_ = [...receivedArguments];
					return spawn(process.execPath, ['-e', 'setInterval(() => {}, 1_000)'], options);
				},
			},
			'/Users/example/.ssh/config',
		);
		const transport = adapter.open({
			askPass: { environment: {} },
			remoteCommand: 'exec runtime attach',
			resolvedTarget: SshEnvironmentResolvedTargetSchema.parse({
				destination: 'build-host',
				hostKeyLookupName: 'build.internal.example',
				hostKeyPolicy: 'ask',
				hostname: '10.0.0.8',
				port: 22,
				user: 'builder',
			}),
			target: SshEnvironmentTargetReferenceSchema.parse({
				configurationFilePath: null,
				destination: 'build-host',
			}),
		});
		const closed = new Promise<void>((resolve) => {
			transport.onExit(() => resolve());
		});

		try {
			expect(arguments_?.slice(-5)).toEqual([
				'-F',
				'/Users/example/.ssh/config',
				'--',
				'build-host',
				'exec runtime attach',
			]);
		} finally {
			transport.retire();
			await closed;
		}
	});

	it('opens one no-PTY stdio transport with forwarding and multiplexing disabled', async () => {
		let executablePath: string | undefined;
		let arguments_: readonly string[] | undefined;
		const adapter = new OpenSshTransportAdapter('/usr/bin/ssh', {
			spawn: (receivedExecutablePath, receivedArguments, options) => {
				executablePath = receivedExecutablePath;
				arguments_ = [...receivedArguments];
				return spawn(process.execPath, ['-e', 'setInterval(() => {}, 1_000)'], options);
			},
		});
		const transport = adapter.open({
			askPass: { environment: { ORCHBEE_ASKPASS_CAPABILITY: 'test-capability' } },
			remoteCommand: 'set -eu\nexec runtime attach',
			resolvedTarget: SshEnvironmentResolvedTargetSchema.parse({
				destination: 'build-host',
				hostKeyLookupName: 'build.internal.example',
				hostKeyPolicy: 'ask',
				hostname: '10.0.0.8',
				port: 2222,
				user: 'builder',
			}),
			target: SshEnvironmentTargetReferenceSchema.parse({
				configurationFilePath: '/Users/example/.ssh/orchbee_config',
				destination: 'build-host',
			}),
		});
		const closed = new Promise<void>((resolve) => {
			transport.onExit(() => resolve());
		});

		try {
			expect(executablePath).toBe('/usr/bin/ssh');
			expect(arguments_?.[0]).toBe('-T');
			expect(arguments_).toContain('ClearAllForwardings=yes');
			expect(arguments_).toContain('ControlMaster=no');
			expect(arguments_).toContain('ControlPath=none');
			expect(arguments_).toContain('ControlPersist=no');
			expect(arguments_).toContain('ForwardAgent=no');
			expect(arguments_).toContain('ForkAfterAuthentication=no');
			expect(arguments_).toContain('ForwardX11=no');
			expect(arguments_).toContain('GSSAPIDelegateCredentials=no');
			expect(arguments_).toContain('PermitLocalCommand=no');
			expect(arguments_).toContain('RequestTTY=no');
			expect(arguments_).toContain('ServerAliveInterval=15');
			expect(arguments_).toContain('ServerAliveCountMax=3');
			expect(arguments_).toContain('StdinNull=no');
			expect(arguments_).toContain('Tunnel=no');
			expect(arguments_).toContain('HostName=10.0.0.8');
			expect(arguments_).toContain('HostKeyAlias=build.internal.example');
			expect(arguments_).toContain('Port=2222');
			expect(arguments_).toContain('User=builder');
			expect(arguments_).not.toContain('-D');
			expect(arguments_?.slice(-2)).toEqual(['build-host', 'set -eu\nexec runtime attach']);
			const separator = arguments_?.lastIndexOf('--') ?? -1;
			expect(arguments_?.slice(separator)).toEqual([
				'--',
				'build-host',
				'set -eu\nexec runtime attach',
			]);
		} finally {
			transport.retire();
			await closed;
		}
	});
});
