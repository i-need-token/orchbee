import { spawnSync } from 'node:child_process';

import {
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentReleaseCohortDigestSchema,
	SshEnvironmentRuntimeVersionSchema,
	SshEnvironmentTerminalHostVersionSchema,
} from '@orchbee/ssh-environment-protocol';
import { describe, expect, it } from 'vitest';

import { createSshEnvironmentRemoteBootstrap } from '../createSshEnvironmentRemoteBootstrap';
import type { VerifiedSshEnvironmentReleaseCatalog } from '../verify-ssh-environment-release';

const runtimeVersion = SshEnvironmentRuntimeVersionSchema.parse('0.1.0');
const darwinDigest = SshEnvironmentBuildDigestSchema.parse('a'.repeat(64));
const linuxDigest = SshEnvironmentBuildDigestSchema.parse('b'.repeat(64));
const terminalHostVersion = SshEnvironmentTerminalHostVersionSchema.parse('0.1.0');
const darwinTerminalHostDigest = SshEnvironmentBuildDigestSchema.parse('c'.repeat(64));
const linuxTerminalHostDigest = SshEnvironmentBuildDigestSchema.parse('d'.repeat(64));
const catalog: VerifiedSshEnvironmentReleaseCatalog = {
	components: {
		'ssh-environment-runtime': {
			artifacts: {
				'darwin-arm64': {
					buildDigest: darwinDigest,
					platform: 'darwin-arm64',
					sizeBytes: 4,
				},
				'linux-x64': {
					buildDigest: linuxDigest,
					platform: 'linux-x64',
					sizeBytes: 8,
				},
			},
			role: 'ssh-environment-runtime',
			version: runtimeVersion,
		},
		'terminal-host': {
			artifacts: {
				'darwin-arm64': {
					buildDigest: darwinTerminalHostDigest,
					platform: 'darwin-arm64',
					sizeBytes: 5,
				},
				'linux-x64': {
					buildDigest: linuxTerminalHostDigest,
					platform: 'linux-x64',
					sizeBytes: 9,
				},
			},
			role: 'terminal-host',
			version: terminalHostVersion,
		},
	},
	releaseCohortDigest: SshEnvironmentReleaseCohortDigestSchema.parse('e'.repeat(64)),
};

describe('SSH Environment remote bootstrap', () => {
	it('builds a syntax-valid dual-platform program with no remote download path', () => {
		const bootstrap = createSshEnvironmentRemoteBootstrap({
			catalog,
			runtimeInstanceId: '1'.repeat(32),
		});

		expect(spawnSync('/bin/sh', ['-n', '-c', bootstrap.command]).status).toBe(0);
		expect(bootstrap.command).toContain('orchbee_directory="$HOME/.orchbee"');
		expect(bootstrap.command).toContain(
			'runtime_version_directory="$runtime_versions_directory/0.1.0"',
		);
		expect(bootstrap.command).toContain(
			'terminal_host_version_directory="$terminal_host_versions_directory/0.1.0"',
		);
		expect(bootstrap.command).toContain(`runtime_expected_digest=${darwinDigest}`);
		expect(bootstrap.command).toContain(`runtime_expected_digest=${linuxDigest}`);
		expect(bootstrap.command).toContain(
			`terminal_host_expected_digest=${darwinTerminalHostDigest}`,
		);
		expect(bootstrap.command).toContain(`terminal_host_expected_digest=${linuxTerminalHostDigest}`);
		expect(bootstrap.command).toContain('runtime_expected_size=4');
		expect(bootstrap.command).toContain('terminal_host_expected_size=9');
		expect(bootstrap.command).toContain('OBSI/2 darwin-arm64 ssh-environment-runtime present');
		expect(bootstrap.command).toContain('OBSI/2 linux-x64 terminal-host upload');
		expect(bootstrap.command).toContain('OBSC/2 darwin-arm64 ready');
		expect(bootstrap.command).toContain('nohup "$runtime_executable" serve');
		expect(bootstrap.command).toContain('--build-digest "$runtime_expected_digest"');
		expect(bootstrap.command).toContain('--terminal-host-executable "$terminal_host_executable"');
		expect(bootstrap.command).toContain('exec "$runtime_executable" attach');
		expect(bootstrap.command).toContain('[ ! -L "$component_executable" ] || exit 78');
		expect(bootstrap.command).toContain('ln "$incoming" "$component_executable"');
		expect(bootstrap.command).not.toMatch(/\bcurl\b|\bwget\b/u);
	});

	it('fails closed before interpolation when a Runtime identity is malformed', () => {
		expect(() =>
			createSshEnvironmentRemoteBootstrap({ catalog, runtimeInstanceId: '$(id)' }),
		).toThrow(/./u);
	});

	it('requires glibc only for the admitted Linux x86_64 branch', () => {
		const bootstrap = createSshEnvironmentRemoteBootstrap({
			catalog,
			runtimeInstanceId: '1'.repeat(32),
		});

		expect(spawnSync('/bin/sh', ['-n', '-c', bootstrap.command]).status).toBe(0);
		expect(bootstrap.command).toContain('Linux:x86_64)');
		expect(bootstrap.command).toContain('getconf GNU_LIBC_VERSION');
	});
});
