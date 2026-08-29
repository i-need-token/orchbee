import { spawnSync } from 'node:child_process';

import {
	SshEnvironmentBuildDigestSchema,
	SshEnvironmentRuntimeVersionSchema,
} from '@orchbee/ssh-environment-protocol';
import { describe, expect, it } from 'vitest';

import { createSshRuntimeRemoteBootstrap } from '../createSshRuntimeRemoteBootstrap';
import type { VerifiedSshRuntimeReleaseCatalog } from '../verify-ssh-runtime-artifact';

const runtimeVersion = SshEnvironmentRuntimeVersionSchema.parse('0.1.0');
const darwinDigest = SshEnvironmentBuildDigestSchema.parse('a'.repeat(64));
const linuxDigest = SshEnvironmentBuildDigestSchema.parse('b'.repeat(64));
const catalog: VerifiedSshRuntimeReleaseCatalog = {
	artifacts: {
		'darwin-arm64': {
			buildDigest: darwinDigest,
			platform: 'darwin-arm64',
			runtimeVersion,
			sizeBytes: 4,
		},
		'linux-x64': {
			buildDigest: linuxDigest,
			platform: 'linux-x64',
			runtimeVersion,
			sizeBytes: 8,
		},
	},
	runtimeVersion,
};

describe('SSH Runtime remote bootstrap', () => {
	it('builds a syntax-valid dual-platform program with no remote download path', () => {
		const bootstrap = createSshRuntimeRemoteBootstrap({
			catalog,
			runtimeInstanceId: '1'.repeat(32),
		});

		expect(spawnSync('/bin/sh', ['-n', '-c', bootstrap.command]).status).toBe(0);
		expect(bootstrap.command).toContain('orchbee_directory="$HOME/.orchbee"');
		expect(bootstrap.command).toContain('version_directory="$versions_directory/0.1.0"');
		expect(bootstrap.command).toContain(`expected_digest=${darwinDigest}`);
		expect(bootstrap.command).toContain(`expected_digest=${linuxDigest}`);
		expect(bootstrap.command).toContain('expected_size=4');
		expect(bootstrap.command).toContain('expected_size=8');
		expect(bootstrap.command).toContain('OBSI/1 darwin-arm64 present');
		expect(bootstrap.command).toContain('OBSI/1 linux-x64 upload');
		expect(bootstrap.command).toContain('nohup "$runtime_executable" serve');
		expect(bootstrap.command).toContain('--build-digest "$expected_digest"');
		expect(bootstrap.command).toContain('exec "$runtime_executable" attach');
		expect(bootstrap.command).toContain('[ ! -L "$runtime_executable" ] || exit 78');
		expect(bootstrap.command).toContain('ln "$incoming" "$runtime_executable"');
		expect(bootstrap.command).not.toMatch(/\bcurl\b|\bwget\b/u);
	});

	it('fails closed before interpolation when a Runtime identity is malformed', () => {
		expect(() => createSshRuntimeRemoteBootstrap({ catalog, runtimeInstanceId: '$(id)' })).toThrow(
			/./u,
		);
	});

	it('requires glibc only for the admitted Linux x86_64 branch', () => {
		const bootstrap = createSshRuntimeRemoteBootstrap({
			catalog,
			runtimeInstanceId: '1'.repeat(32),
		});

		expect(spawnSync('/bin/sh', ['-n', '-c', bootstrap.command]).status).toBe(0);
		expect(bootstrap.command).toContain('Linux:x86_64)');
		expect(bootstrap.command).toContain('getconf GNU_LIBC_VERSION');
	});
});
