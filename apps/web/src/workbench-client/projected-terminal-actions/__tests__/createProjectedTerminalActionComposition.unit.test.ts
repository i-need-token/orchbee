import { AttachmentIdSchema, CanonicalUint64TextSchema } from '@orchbee/terminal-protocol';
import type { TerminalSearchOptions } from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import { TerminalFindController } from '../../terminal-find-surface';
import { createProjectedTerminalActionComposition } from '../createProjectedTerminalActionComposition';
import { PROJECTED_TERMINAL_ACTION_IDS } from '../projected-terminal-action-ids';
import type { ProjectedTerminalActionTarget } from '../registerProjectedTerminalActions';

describe('createProjectedTerminalActionComposition', () => {
	it('commits clipboard success only after the Main-owned effect completes', async () => {
		const terminal = terminalTarget();
		const controller = findController();
		const composition = createProjectedTerminalActionComposition(terminal, controller);
		const execution = composition.execute(
			PROJECTED_TERMINAL_ACTION_IDS.copySelection,
			undefined,
			composition.terminalScope,
			'keyboard',
		);

		await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
		expect(terminal.copySelectionAsync).toHaveBeenCalledOnce();
		expect(controller.getSnapshot().notice).toEqual({
			byteLength: 8,
			kind: 'copy-completed',
		});
		composition.dispose();
	});

	it('exposes blocked file targets as disabled link Actions', async () => {
		const terminal = terminalTarget();
		const controller = findController();
		const composition = createProjectedTerminalActionComposition(terminal, controller);
		const execution = composition.execute(
			PROJECTED_TERMINAL_ACTION_IDS.openLink,
			{ target: { kind: 'file', path: '/tmp/private' } },
			composition.terminalScope,
			'pointer',
		);

		await expect(execution.completion).resolves.toMatchObject({
			code: 'action-unavailable',
			kind: 'blocked',
		});
		expect(terminal.openLinkAsync).not.toHaveBeenCalled();
		composition.dispose();
	});
});

function findController(): TerminalFindController {
	return new TerminalFindController({
		clearSearch: vi.fn<() => void>(),
		updateSearch: vi.fn<(query: string, options: TerminalSearchOptions) => void>(),
	});
}

function terminalTarget(): ProjectedTerminalActionTarget {
	return {
		canControlTerminal: true,
		copySelectionAsync: vi
			.fn<ProjectedTerminalActionTarget['copySelectionAsync']>()
			.mockResolvedValue({ byteLength: 8, kind: 'completed' }),
		findNext: vi.fn<ProjectedTerminalActionTarget['findNext']>(() => true),
		findPrevious: vi.fn<ProjectedTerminalActionTarget['findPrevious']>(() => true),
		openLinkAsync: vi
			.fn<ProjectedTerminalActionTarget['openLinkAsync']>()
			.mockResolvedValue({ kind: 'completed' }),
		reset: vi.fn<ProjectedTerminalActionTarget['reset']>(),
		selectAll: vi.fn<ProjectedTerminalActionTarget['selectAll']>(),
		selection: {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			end: { column: 1, row: CanonicalUint64TextSchema.parse('1') },
			preview: null,
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('1') },
			state: 'active',
		},
	};
}
