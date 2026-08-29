import type { AgentIntegrationCommandResult } from './NodeAgentIntegrationCommandRunner';
import type { CliAgentIntegrationOperationContext } from './CliAgentIntegrationAdapter';
import type { AgentIntegrationMutationAttempt } from './agent-integration-adapter';

export function commandAttempt(
	result: AgentIntegrationCommandResult,
): AgentIntegrationMutationAttempt {
	if (result.kind === 'completed') return { kind: 'attempted' };
	return result.kind === 'timed-out' ? { kind: 'manual-action-required' } : { kind: 'failed' };
}

export async function runCommandSequence(
	context: CliAgentIntegrationOperationContext,
	steps: readonly (readonly string[])[],
): Promise<AgentIntegrationMutationAttempt> {
	for (const commandArguments of steps) {
		const result = commandAttempt(
			await context.commandRunner.run(context.executablePath, commandArguments),
		);
		if (result.kind !== 'attempted') return result;
	}
	return { kind: 'attempted' };
}
