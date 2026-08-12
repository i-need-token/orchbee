import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import type { TerminalSearchOptions } from '@orchbee/terminal';
import { AttachmentIdSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { TerminalFindController } from '../../terminal-find-surface';
import {
	PROJECTED_TERMINAL_ACTION_IDS,
	type ProjectedTerminalActionCatalog,
} from '../projected-terminal-action-ids';
import {
	registerProjectedTerminalActions,
	type ProjectedTerminalActionTarget,
} from '../registerProjectedTerminalActions';

describe('registerProjectedTerminalActions', () => {
	it('routes search navigation and selection through scoped semantic Actions', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('terminal.projected');
		const controller = new TerminalFindController({
			clearSearch: vi.fn<() => void>(),
			updateSearch: vi.fn<(query: string, options: TerminalSearchOptions) => void>(),
		});
		const terminal = terminalTarget();
		const registrations = registerProjectedTerminalActions({
			findController: controller,
			scope,
			terminal,
			typedRuntime: createTypedActionRuntime<ProjectedTerminalActionCatalog>(runtime),
		});

		await expect(
			runtime.execute(PROJECTED_TERMINAL_ACTION_IDS.findOpen, undefined, {
				scope,
				source: 'keyboard',
			}).completion,
		).resolves.toEqual({ kind: 'completed' });
		controller.setQuery('needle');
		await expect(
			runtime.execute(PROJECTED_TERMINAL_ACTION_IDS.findNext, undefined, {
				scope,
				source: 'keyboard',
			}).completion,
		).resolves.toEqual({ kind: 'completed' });
		await expect(
			runtime.execute(PROJECTED_TERMINAL_ACTION_IDS.selectAll, undefined, {
				scope,
				source: 'keyboard',
			}).completion,
		).resolves.toEqual({ kind: 'completed' });
		await expect(
			runtime.execute(PROJECTED_TERMINAL_ACTION_IDS.reset, undefined, {
				scope,
				source: 'menu',
			}).completion,
		).resolves.toEqual({ kind: 'completed' });

		expect(terminal.findNext).toHaveBeenCalledOnce();
		expect(terminal.reset).toHaveBeenCalledOnce();
		expect(terminal.selectAll).toHaveBeenCalledOnce();
		for (const registration of registrations) registration.dispose();
		runtime.dispose();
	});
});

function terminalTarget(): ProjectedTerminalActionTarget {
	return {
		canControlTerminal: true,
		copySelectionAsync: vi
			.fn<ProjectedTerminalActionTarget['copySelectionAsync']>()
			.mockResolvedValue({ kind: 'completed' }),
		findNext: vi.fn<ProjectedTerminalActionTarget['findNext']>(() => true),
		findPrevious: vi.fn<ProjectedTerminalActionTarget['findPrevious']>(() => true),
		openLinkAsync: vi
			.fn<ProjectedTerminalActionTarget['openLinkAsync']>()
			.mockResolvedValue({ kind: 'completed' }),
		reset: vi.fn<ProjectedTerminalActionTarget['reset']>(),
		selectAll: vi.fn<ProjectedTerminalActionTarget['selectAll']>(),
		selection: { attachmentId: AttachmentIdSchema.parse('11'.repeat(16)), state: 'none' },
	};
}
