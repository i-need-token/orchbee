import { AttachmentIdSchema, CanonicalUint64TextSchema } from '@orchbee/terminal-protocol';
import type { TerminalSearchOptions } from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import { DesktopTerminalFindController } from '../../terminal-find-surface';
import { createDesktopTerminalActionComposition } from '../createDesktopTerminalActionComposition';
import { DESKTOP_TERMINAL_ACTION_IDS } from '../desktop-terminal-action-ids';
import type { DesktopTerminalActionTarget } from '../registerDesktopTerminalActions';

describe('createDesktopTerminalActionComposition', () => {
	it('commits clipboard success only after the Main-owned effect completes', async () => {
		const terminal = terminalTarget();
		const controller = findController();
		const composition = createDesktopTerminalActionComposition(terminal, controller);
		const execution = composition.execute(
			DESKTOP_TERMINAL_ACTION_IDS.copySelection,
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
		const composition = createDesktopTerminalActionComposition(terminal, controller);
		const execution = composition.execute(
			DESKTOP_TERMINAL_ACTION_IDS.openLink,
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

function findController(): DesktopTerminalFindController {
	return new DesktopTerminalFindController({
		clearSearch: vi.fn<() => void>(),
		updateSearch: vi.fn<(query: string, options: TerminalSearchOptions) => void>(),
	});
}

function terminalTarget(): DesktopTerminalActionTarget {
	return {
		copySelectionAsync: vi
			.fn<DesktopTerminalActionTarget['copySelectionAsync']>()
			.mockResolvedValue({ byteLength: 8, kind: 'completed' }),
		findNext: vi.fn<DesktopTerminalActionTarget['findNext']>(() => true),
		findPrevious: vi.fn<DesktopTerminalActionTarget['findPrevious']>(() => true),
		openLinkAsync: vi
			.fn<DesktopTerminalActionTarget['openLinkAsync']>()
			.mockResolvedValue({ kind: 'completed' }),
		selectAll: vi.fn<DesktopTerminalActionTarget['selectAll']>(),
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
