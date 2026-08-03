import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import type { TerminalSearchOptions } from '@orchbee/terminal';
import { AttachmentIdSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { DesktopTerminalFindController } from '../../terminal-find-surface';
import {
	DESKTOP_TERMINAL_ACTION_IDS,
	type DesktopTerminalActionCatalog,
} from '../desktop-terminal-action-ids';
import {
	registerDesktopTerminalActions,
	type DesktopTerminalActionTarget,
} from '../registerDesktopTerminalActions';

describe('registerDesktopTerminalActions', () => {
	it('routes search navigation and selection through scoped semantic Actions', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('desktop.terminal');
		const controller = new DesktopTerminalFindController({
			clearSearch: vi.fn<() => void>(),
			updateSearch: vi.fn<(query: string, options: TerminalSearchOptions) => void>(),
		});
		const terminal = terminalTarget();
		const registrations = registerDesktopTerminalActions({
			findController: controller,
			scope,
			terminal,
			typedRuntime: createTypedActionRuntime<DesktopTerminalActionCatalog>(runtime),
		});

		await expect(
			runtime.execute(DESKTOP_TERMINAL_ACTION_IDS.findOpen, undefined, {
				scope,
				source: 'keyboard',
			}).completion,
		).resolves.toEqual({ kind: 'completed' });
		controller.setQuery('needle');
		await expect(
			runtime.execute(DESKTOP_TERMINAL_ACTION_IDS.findNext, undefined, {
				scope,
				source: 'keyboard',
			}).completion,
		).resolves.toEqual({ kind: 'completed' });
		await expect(
			runtime.execute(DESKTOP_TERMINAL_ACTION_IDS.selectAll, undefined, {
				scope,
				source: 'keyboard',
			}).completion,
		).resolves.toEqual({ kind: 'completed' });

		expect(terminal.findNext).toHaveBeenCalledOnce();
		expect(terminal.selectAll).toHaveBeenCalledOnce();
		for (const registration of registrations) registration.dispose();
		runtime.dispose();
	});
});

function terminalTarget(): DesktopTerminalActionTarget {
	return {
		copySelectionAsync: vi
			.fn<DesktopTerminalActionTarget['copySelectionAsync']>()
			.mockResolvedValue({ kind: 'completed' }),
		findNext: vi.fn<DesktopTerminalActionTarget['findNext']>(() => true),
		findPrevious: vi.fn<DesktopTerminalActionTarget['findPrevious']>(() => true),
		openLinkAsync: vi
			.fn<DesktopTerminalActionTarget['openLinkAsync']>()
			.mockResolvedValue({ kind: 'completed' }),
		selectAll: vi.fn<DesktopTerminalActionTarget['selectAll']>(),
		selection: { attachmentId: AttachmentIdSchema.parse('11'.repeat(16)), state: 'none' },
	};
}
