import { describe, expect, it } from 'vitest';

import {
	SSH_RUNTIME_BOOTSTRAP_STATUS_LINES,
	SshRuntimeBootstrapStatusDecoder,
} from '../ssh-runtime-bootstrap-negotiation';

describe('SSH Runtime bootstrap negotiation', () => {
	it.each([
		['darwin-arm64', 'present'],
		['darwin-arm64', 'upload'],
		['linux-x64', 'present'],
		['linux-x64', 'upload'],
	] as const)('decodes the exact %s %s status across chunks', (platform, disposition) => {
		const decoder = new SshRuntimeBootstrapStatusDecoder();
		const line = `${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES[platform][disposition]}\n`;
		expect(decoder.push(Buffer.from(line.slice(0, 5)))).toEqual({ kind: 'pending' });
		expect(decoder.push(Buffer.from(line.slice(5)))).toEqual({
			kind: 'status',
			status: { artifactDisposition: disposition, platform },
		});
	});

	it.each([
		Buffer.from('OBSI/1 darwin-arm64 missing\n'),
		Buffer.from(`${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['darwin-arm64'].present}\nextra`),
		Buffer.alloc(129, 0x78),
		Buffer.from([0xff, 0x0a]),
	])('rejects malformed, extra, oversized, or non-ASCII status bytes', (bytes) => {
		const decoder = new SshRuntimeBootstrapStatusDecoder();
		expect(decoder.push(bytes)).toEqual({ kind: 'rejected' });
	});

	it('accepts only one status', () => {
		const decoder = new SshRuntimeBootstrapStatusDecoder();
		const line = Buffer.from(`${SSH_RUNTIME_BOOTSTRAP_STATUS_LINES['linux-x64'].present}\n`);
		expect(decoder.push(line).kind).toBe('status');
		expect(decoder.push(line)).toEqual({ kind: 'rejected' });
	});
});
