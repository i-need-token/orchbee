import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { TerminalPresentationIdSchema } from '@orchbee/workbench-client-protocol';
import { expect, it, vi } from 'vitest';

import type {
	WorkbenchAgentConfirmation,
	WorkbenchAgentConfirmations,
} from '../../types/workbench-agent-confirmations';
import { registerWorkbenchAgentConfirmationAction } from '../registerWorkbenchAgentConfirmationAction';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchAgentConfirmationChooseInput,
} from '../workbench-action-ids';

// A wrong mapping here either turns answering on for more than the person chose or keeps offering
// after their No, so each answer must reach the Server as the request that means it.
it('sends each answer as the request that means it, and only for an Agent the Server lists', async () => {
	const listed = TerminalPresentationIdSchema.parse('terminal-listed');
	const gone = TerminalPresentationIdSchema.parse('terminal-gone');
	const row: WorkbenchAgentConfirmation = {
		agent: 'claude',
		choice: 'off',
		offer: true,
		pressed: null,
		stopped: false,
		wait: null,
	};
	const choose = vi.fn<WorkbenchAgentConfirmations['choose']>(async () => true);
	const decline = vi.fn<WorkbenchAgentConfirmations['decline']>(async () => false);
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('workbench');
	registerWorkbenchAgentConfirmationAction({
		confirmations: {
			choose,
			decline,
			getSnapshot: () => new Map([[listed, row]]),
			subscribe: () => () => undefined,
		},
		scope,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(runtime),
	});
	const execute = (input: WorkbenchAgentConfirmationChooseInput) =>
		runtime.execute(WORKBENCH_ACTION_IDS.chooseAgentConfirmation, input, { scope, source: 'menu' })
			.completion;

	for (const answer of ['off', 'run', 'agent-type'] as const) {
		await expect(execute({ answer, presentationId: listed })).resolves.toEqual({
			kind: 'completed',
		});
		expect(choose).toHaveBeenLastCalledWith(listed, answer);
	}
	expect(decline).not.toHaveBeenCalled();

	// The Server did not apply it: nothing changed here, and the person is told.
	await expect(execute({ answer: 'no', presentationId: listed })).resolves.toEqual({
		failure: { code: 'agent-confirmation-not-changed' },
		kind: 'failed',
	});
	expect(decline).toHaveBeenCalledExactlyOnceWith(listed);
	expect(choose).toHaveBeenCalledTimes(3);

	await expect(execute({ answer: 'run', presentationId: gone })).resolves.toMatchObject({
		kind: 'blocked',
	});
	expect(choose).toHaveBeenCalledTimes(3);
});
