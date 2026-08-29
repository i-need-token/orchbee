import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { Readable } from 'node:stream';

import { SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES } from '@orchbee/semantic-command-protocol/local-http';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	runSemanticCommandCliProcess,
	startSemanticCommandCliProcess,
	type SemanticCommandCliProcessHost,
} from '../semantic-command-cli-process';

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories
			.splice(0)
			.map((directory) => rm(directory, { force: true, recursive: true })),
	);
});

describe('semantic command CLI process', () => {
	it('projects a missing or relative product connection as coordinator unavailable', async () => {
		for (const environment of [
			{},
			{
				ORCHBEE_CLI_NATIVE_ADDON_PATH: 'relative/addon.node',
				ORCHBEE_CLI_RUNTIME_DIRECTORY: 'relative/runtime',
			},
		]) {
			const result = await runSemanticCommandCliProcess({
				arguments: ['status'],
				environment,
				stdin: Readable.from([]),
			});

			expect(result.exitCode).toBe(6);
			expect(JSON.parse(result.stdout)).toMatchObject({
				code: 'coordinator-unavailable',
				command: 'status',
				kind: 'error',
			});
			expect(result.stderr).toBe('');
		}
	});

	it('reads valid invocation input from stdin before creating one request identity', async () => {
		const result = await runSemanticCommandCliProcess({
			arguments: ['operation', 'invoke', 'workbench.project.list', '--input-file', '-'],
			environment: {},
			stdin: Readable.from(['{"limit":1}']),
		});

		expect(result.exitCode).toBe(6);
		expect(JSON.parse(result.stdout)).toMatchObject({
			code: 'coordinator-unavailable',
			command: 'operation.invoke',
			kind: 'error',
			requestIdentity: expect.stringMatching(/^cli:request:[0-9a-f-]{36}$/u),
		});
	});

	it('reads one bounded file without loading trailing over-capacity bytes', async () => {
		const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-cli-process-'));
		temporaryDirectories.push(directory);
		const inputPath = path.join(directory, 'input.json');
		await writeFile(
			inputPath,
			Buffer.alloc(SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES + 2, 0x20),
		);

		const result = await runSemanticCommandCliProcess({
			arguments: ['operation', 'invoke', 'workbench.project.list', '--input-file', inputPath],
			environment: {},
			stdin: Readable.from([]),
		});

		expect(result.exitCode).toBe(2);
		expect(JSON.parse(result.stdout)).toMatchObject({
			code: 'input-capacity-exceeded',
			command: 'operation.invoke',
			kind: 'error',
		});
	});

	it('caps stdin before the runner validates invocation capacity', async () => {
		const result = await runSemanticCommandCliProcess({
			arguments: ['operation', 'invoke', 'workbench.project.list', '--input-file', '-'],
			environment: {},
			stdin: Readable.from([
				Buffer.alloc(SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES + 1, 0x20),
			]),
		});

		expect(result.exitCode).toBe(2);
		expect(JSON.parse(result.stdout)).toMatchObject({
			code: 'input-capacity-exceeded',
			kind: 'error',
		});
	});

	it('writes exactly one result and releases process signal handlers', async () => {
		const writes: string[] = [];
		const removeAbortHandlers = vi.fn<() => void>();
		const setExitCode = vi.fn<SemanticCommandCliProcessHost['setExitCode']>();
		const registerAbort = vi.fn<SemanticCommandCliProcessHost['registerAbort']>(
			() => removeAbortHandlers,
		);
		await startSemanticCommandCliProcess({
			arguments: ['unknown'],
			environment: {},
			registerAbort,
			setExitCode,
			stdin: Readable.from([]),
			writeStderr: (value) => writes.push(`stderr:${value}`),
			writeStdout: (value) => writes.push(value),
		});

		expect(writes).toHaveLength(1);
		expect(JSON.parse(writes[0] ?? '')).toMatchObject({
			code: 'cli-usage-invalid',
			kind: 'error',
		});
		expect(registerAbort).toHaveBeenCalledOnce();
		expect(removeAbortHandlers).toHaveBeenCalledOnce();
		expect(setExitCode).toHaveBeenCalledWith(2);
	});
});
