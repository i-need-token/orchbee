import { describe, expect, it } from 'vitest';

import {
	cliErrorResult,
	cliFailureResult,
	cliProtocolResult,
} from '../semantic-command-cli-output';

describe('semantic command CLI output', () => {
	it('writes one compact versioned JSON document for machine output', () => {
		const result = cliProtocolResult(
			'status',
			{ coordinatorGeneration: 'generation-1', kind: 'ready', protocolVersion: 1 },
			0,
			'json',
		);

		expect(result).toEqual({
			exitCode: 0,
			stderr: '',
			stdout:
				'{"cliVersion":1,"command":"status","kind":"result","result":{"coordinatorGeneration":"generation-1","kind":"ready","protocolVersion":1}}\n',
		});
	});

	it('keeps request identity in failures without copying operation payloads to diagnostics', () => {
		const result = cliFailureResult(
			'operation.invoke',
			'outcome-unknown-after-restart',
			6,
			'json',
			'cli:request:1',
		);

		expect(JSON.parse(result.stdout)).toEqual({
			cliVersion: 1,
			code: 'outcome-unknown-after-restart',
			command: 'operation.invoke',
			kind: 'error',
			requestIdentity: 'cli:request:1',
		});
		expect(result.stderr).toBe('');
	});

	it('uses pretty JSON only as the explicitly unstable text projection', () => {
		const result = cliErrorResult(null, 'cli-usage-invalid', 'text');
		expect(result.exitCode).toBe(2);
		expect(result.stdout).toContain('\n  "cliVersion": 1,');
		expect(JSON.parse(result.stdout)).toEqual({
			cliVersion: 1,
			code: 'cli-usage-invalid',
			command: null,
			kind: 'error',
		});
	});
});
