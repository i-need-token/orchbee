import { describe, expect, it } from 'vitest';

import type {
	AgentIntegrationCommandResult,
	AgentIntegrationCommandRunner,
} from '../NodeAgentIntegrationCommandRunner';
import { commandAttempt, runCommandSequence } from '../agent-integration-command-operations';

describe('Agent integration command operations', () => {
	it('maps command completion, timeout, and failure without exposing command output', () => {
		expect(commandAttempt({ kind: 'completed', stdout: 'private output' })).toEqual({
			kind: 'attempted',
		});
		expect(commandAttempt({ kind: 'timed-out' })).toEqual({
			kind: 'manual-action-required',
		});
		expect(commandAttempt({ kind: 'output-too-large' })).toEqual({ kind: 'failed' });
	});

	it('runs ordered steps and stops at the first unsuccessful command', async () => {
		const runner = new SequenceRunner([
			{ kind: 'completed', stdout: '' },
			{ kind: 'failed' },
			{ kind: 'completed', stdout: '' },
		]);

		await expect(
			runCommandSequence(
				{ commandRunner: runner, executablePath: '/bin/agent', resourceRoot: '/resource' },
				[['first'], ['second'], ['third']],
			),
		).resolves.toEqual({ kind: 'failed' });
		expect(runner.calls).toEqual([['first'], ['second']]);
	});
});

class SequenceRunner implements AgentIntegrationCommandRunner {
	readonly calls: (readonly string[])[] = [];
	readonly #results: AgentIntegrationCommandResult[];

	constructor(results: AgentIntegrationCommandResult[]) {
		this.#results = [...results];
	}

	resolveExecutable(): Promise<string | null> {
		return Promise.resolve('/bin/agent');
	}

	run(
		_executablePath: string,
		commandArguments: readonly string[],
	): Promise<AgentIntegrationCommandResult> {
		this.calls.push(commandArguments);
		return Promise.resolve(this.#results.shift() ?? { kind: 'failed' });
	}
}
