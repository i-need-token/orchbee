import { describe, expect, it } from 'vitest';

import { NodeAgentIntegrationCommandRunner } from '../NodeAgentIntegrationCommandRunner';

describe('NodeAgentIntegrationCommandRunner', () => {
	it('resolves only bounded executable names to absolute paths', async () => {
		const runner = createRunner();

		expect(await runner.resolveExecutable('sh')).toMatch(/^\//u);
		await expect(runner.resolveExecutable('../sh')).resolves.toBeNull();
		await expect(runner.resolveExecutable('SH')).resolves.toBeNull();
	});

	it('runs only absolute executables with ignored stdin and bounded stdout', async () => {
		const runner = createRunner();

		await expect(runner.run('/bin/sh', ['-c', "printf 'ready'"])).resolves.toEqual({
			kind: 'completed',
			stdout: 'ready',
		});
		await expect(runner.run('sh', ['-c', "printf 'ignored'"])).resolves.toEqual({
			kind: 'failed',
		});
	});
});

function createRunner(): NodeAgentIntegrationCommandRunner {
	return new NodeAgentIntegrationCommandRunner({
		environment: process.env,
		homeDirectory: process.cwd(),
		shellExecutable: '/bin/sh',
	});
}
