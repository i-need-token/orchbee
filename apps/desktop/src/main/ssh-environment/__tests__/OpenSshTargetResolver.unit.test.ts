import { describe, expect, it, vi } from 'vitest';

import { SshEnvironmentTargetReferenceSchema } from '../ssh-environment-contract';
import { OpenSshTargetResolver } from '../OpenSshTargetResolver';

type OpenSshTargetResolverDependencies = NonNullable<
	ConstructorParameters<typeof OpenSshTargetResolver>[1]
>;

const directTarget = SshEnvironmentTargetReferenceSchema.parse({
	configurationFilePath: null,
	destination: 'build-host',
});

describe('OpenSshTargetResolver', () => {
	it('selects only a fixed system OpenSSH location for each platform family', () => {
		expect(OpenSshTargetResolver.resolveSystemExecutablePath('darwin', undefined)).toBe(
			'/usr/bin/ssh',
		);
		expect(OpenSshTargetResolver.resolveSystemExecutablePath('linux', undefined)).toBe(
			'/usr/bin/ssh',
		);
		expect(OpenSshTargetResolver.resolveSystemExecutablePath('win32', 'C:\\Windows')).toBe(
			'C:\\Windows\\System32\\OpenSSH\\ssh.exe',
		);
		expect(OpenSshTargetResolver.resolveSystemExecutablePath('win32', undefined)).toBe('ssh');
	});

	it('queries effective OpenSSH configuration without a shell and admits strict host identity', async () => {
		const dependencies = createDependencies({
			kind: 'completed',
			stdout: configurationOutput({
				hostKeyAlias: 'build.internal.example',
				strictHostKeyChecking: 'ask',
			}),
		});
		const resolver = new OpenSshTargetResolver('/usr/bin/ssh', dependencies);

		await expect(resolver.resolve(directTarget)).resolves.toEqual({
			kind: 'resolved',
			target: {
				destination: 'build-host',
				hostKeyLookupName: 'build.internal.example',
				hostKeyPolicy: 'ask',
				hostname: '10.0.0.8',
				port: 22,
				user: 'builder',
			},
		});
		expect(dependencies.query).toHaveBeenCalledWith('/usr/bin/ssh', ['-G', '--', 'build-host']);
	});

	it('uses an available application-home configuration for a target without an override', async () => {
		const dependencies = {
			...createDependencies({
				kind: 'completed' as const,
				stdout: configurationOutput({ strictHostKeyChecking: 'ask' }),
			}),
			fileExists: vi.fn<(path: string) => boolean>(() => true),
		};
		const resolver = new OpenSshTargetResolver(
			'/usr/bin/ssh',
			dependencies,
			'/Users/example/.ssh/config',
		);

		await expect(resolver.resolve(directTarget)).resolves.toMatchObject({ kind: 'resolved' });
		expect(dependencies.fileExists).toHaveBeenCalledWith('/Users/example/.ssh/config');
		expect(dependencies.query).toHaveBeenCalledWith('/usr/bin/ssh', [
			'-G',
			'-F',
			'/Users/example/.ssh/config',
			'--',
			'build-host',
		]);
	});

	it('passes one explicit absolute configuration file before the protected destination', async () => {
		const dependencies = createDependencies({
			kind: 'completed',
			stdout: configurationOutput({ strictHostKeyChecking: 'true' }),
		});
		const resolver = new OpenSshTargetResolver('/opt/openssh/bin/ssh', dependencies);
		const target = SshEnvironmentTargetReferenceSchema.parse({
			configurationFilePath: '/Users/example/.ssh/orchbee_config',
			destination: 'build-host',
		});

		const result = await resolver.resolve(target);

		expect(result).toMatchObject({ kind: 'resolved', target: { hostKeyPolicy: 'yes' } });
		expect(dependencies.query).toHaveBeenCalledWith('/opt/openssh/bin/ssh', [
			'-G',
			'-F',
			'/Users/example/.ssh/orchbee_config',
			'--',
			'build-host',
		]);
	});

	it('uses the effective hostname when OpenSSH supplies no HostKeyAlias', async () => {
		const resolver = new OpenSshTargetResolver(
			'/usr/bin/ssh',
			createDependencies({
				kind: 'completed',
				stdout: configurationOutput({ strictHostKeyChecking: 'accept-new' }),
			}),
		);

		await expect(resolver.resolve(directTarget)).resolves.toMatchObject({
			kind: 'resolved',
			target: {
				hostKeyLookupName: '10.0.0.8',
				hostKeyPolicy: 'accept-new',
			},
		});
	});

	it.each(['false', 'no', 'off'])(
		'fails closed when the effective configuration disables host-key checking with %s',
		async (strictHostKeyChecking) => {
			const resolver = new OpenSshTargetResolver(
				'/usr/bin/ssh',
				createDependencies({
					kind: 'completed',
					stdout: configurationOutput({ strictHostKeyChecking }),
				}),
			);

			await expect(resolver.resolve(directTarget)).resolves.toEqual({
				code: 'host-key-checking-disabled',
				kind: 'failed',
			});
		},
	);

	it('accepts the documented yes spelling in addition to OpenSSH normalized true', async () => {
		const resolver = new OpenSshTargetResolver(
			'/usr/bin/ssh',
			createDependencies({
				kind: 'completed',
				stdout: configurationOutput({ strictHostKeyChecking: 'yes' }),
			}),
		);

		await expect(resolver.resolve(directTarget)).resolves.toMatchObject({
			kind: 'resolved',
			target: { hostKeyPolicy: 'yes' },
		});
	});

	it('rejects non-absolute trusted executable and configuration paths before process execution', async () => {
		const dependencies = createDependencies({
			kind: 'completed',
			stdout: configurationOutput({ strictHostKeyChecking: 'ask' }),
		});
		const relativeExecutable = new OpenSshTargetResolver('ssh', dependencies);
		await expect(relativeExecutable.resolve(directTarget)).resolves.toEqual({
			code: 'openssh-path-not-absolute',
			kind: 'failed',
		});

		const relativeConfiguration = SshEnvironmentTargetReferenceSchema.parse({
			configurationFilePath: '.ssh/config',
			destination: 'build-host',
		});
		const resolver = new OpenSshTargetResolver('/usr/bin/ssh', dependencies);
		await expect(resolver.resolve(relativeConfiguration)).resolves.toEqual({
			code: 'configuration-path-not-absolute',
			kind: 'failed',
		});
		expect(dependencies.query).not.toHaveBeenCalled();
	});

	it.each([
		['executable-unavailable', 'openssh-unavailable'],
		['output-limit', 'openssh-output-too-large'],
		['query-failed', 'openssh-query-failed'],
		['timed-out', 'openssh-query-timed-out'],
	] as const)('maps the bounded process failure %s without diagnostics', async (reason, code) => {
		const resolver = new OpenSshTargetResolver(
			'/usr/bin/ssh',
			createDependencies({ kind: 'failed', reason }),
		);

		await expect(resolver.resolve(directTarget)).resolves.toEqual({ code, kind: 'failed' });
	});

	it.each([
		'',
		'hostname 10.0.0.8\nuser builder\nport 22\n',
		'hostname 10.0.0.8\nhostname other\nuser builder\nport 22\nstricthostkeychecking ask\n',
		'hostname 10.0.0.8\nuser builder\nport invalid\nstricthostkeychecking ask\n',
		'hostname host with spaces\nuser builder\nport 22\nstricthostkeychecking ask\n',
	])('rejects malformed or ambiguous OpenSSH output without returning it', async (stdout) => {
		const resolver = new OpenSshTargetResolver(
			'/usr/bin/ssh',
			createDependencies({ kind: 'completed', stdout }),
		);

		await expect(resolver.resolve(directTarget)).resolves.toEqual({
			code: 'openssh-output-invalid',
			kind: 'failed',
		});
	});

	it('independently rejects an oversized injected result', async () => {
		const resolver = new OpenSshTargetResolver(
			'/usr/bin/ssh',
			createDependencies({ kind: 'completed', stdout: 'x'.repeat(256 * 1_024 + 1) }),
		);

		await expect(resolver.resolve(directTarget)).resolves.toEqual({
			code: 'openssh-output-too-large',
			kind: 'failed',
		});
	});
});

function createDependencies(
	result: Awaited<ReturnType<OpenSshTargetResolverDependencies['query']>>,
): OpenSshTargetResolverDependencies & {
	readonly query: ReturnType<typeof vi.fn<OpenSshTargetResolverDependencies['query']>>;
} {
	return {
		isAbsolute: (path) => path.startsWith('/'),
		query: vi.fn<OpenSshTargetResolverDependencies['query']>().mockResolvedValue(result),
	};
}

function configurationOutput(options: {
	readonly hostKeyAlias?: string;
	readonly strictHostKeyChecking: string;
}): string {
	return [
		'host build-host',
		'user builder',
		'hostname 10.0.0.8',
		'port 22',
		`stricthostkeychecking ${options.strictHostKeyChecking}`,
		...(options.hostKeyAlias === undefined ? [] : [`hostkeyalias ${options.hostKeyAlias}`]),
		'forwardagent no',
		'',
	].join('\n');
}
