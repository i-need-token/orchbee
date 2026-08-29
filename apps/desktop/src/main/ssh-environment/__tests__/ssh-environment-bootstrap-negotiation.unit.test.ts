import { describe, expect, it } from 'vitest';

import {
	SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES,
	SshEnvironmentBootstrapDecoder,
} from '../ssh-environment-bootstrap-negotiation';

describe('SSH Environment bootstrap cohort negotiation', () => {
	it.each([
		['darwin-arm64', 'ssh-environment-runtime', 'present'],
		['darwin-arm64', 'terminal-host', 'upload'],
		['linux-x64', 'ssh-environment-runtime', 'upload'],
		['linux-x64', 'terminal-host', 'present'],
	] as const)('decodes the exact %s %s %s status across chunks', (platform, role, disposition) => {
		const decoder = new SshEnvironmentBootstrapDecoder();
		const line = `${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES[platform][role][disposition]}\n`;
		expect(decoder.push(Buffer.from(line.slice(0, 5)))).toEqual({
			kind: 'messages',
			messages: [],
		});
		expect(decoder.push(Buffer.from(line.slice(5)))).toEqual({
			kind: 'messages',
			messages: [{ artifactDisposition: disposition, platform, role, type: 'artifact-status' }],
		});
	});

	it('decodes both component statuses and the commit barrier from one chunk', () => {
		const decoder = new SshEnvironmentBootstrapDecoder();
		const lines = SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64'];
		expect(
			decoder.push(
				Buffer.from(
					`${lines['ssh-environment-runtime'].present}\n${lines['terminal-host'].upload}\n${lines.committed}\n`,
				),
			),
		).toEqual({
			kind: 'messages',
			messages: [
				{
					artifactDisposition: 'present',
					platform: 'darwin-arm64',
					role: 'ssh-environment-runtime',
					type: 'artifact-status',
				},
				{
					artifactDisposition: 'upload',
					platform: 'darwin-arm64',
					role: 'terminal-host',
					type: 'artifact-status',
				},
				{ platform: 'darwin-arm64', type: 'cohort-committed' },
			],
		});
	});

	it.each([
		Buffer.from('OBSI/1 darwin-arm64 missing\n'),
		Buffer.from(`${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['darwin-arm64'].committed}\nextra`),
		Buffer.alloc(129, 0x78),
		Buffer.from([0xff, 0x0a]),
	])('rejects malformed, post-commit, oversized, or non-ASCII bytes', (bytes) => {
		const decoder = new SshEnvironmentBootstrapDecoder();
		expect(decoder.push(bytes)).toEqual({ kind: 'rejected' });
	});

	it('rejects all input after the commit barrier', () => {
		const decoder = new SshEnvironmentBootstrapDecoder();
		const commit = Buffer.from(
			`${SSH_ENVIRONMENT_BOOTSTRAP_STATUS_LINES['linux-x64'].committed}\n`,
		);
		expect(decoder.push(commit).kind).toBe('messages');
		expect(decoder.push(commit)).toEqual({ kind: 'rejected' });
	});
});
