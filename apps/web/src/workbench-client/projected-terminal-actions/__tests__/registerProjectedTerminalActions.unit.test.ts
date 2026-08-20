import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import type { TerminalSearchOptions } from '@orchbee/terminal';
import { AttachmentIdSchema, CanonicalUint64TextSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { TerminalFindController } from '../../terminal-find-surface';
import type { ProjectedTerminalPlatformEffects } from '../../projected-terminal-platform-effects';
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

	it('seeds find from the bounded active selection preview', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('terminal.projected');
		const updateSearch = vi.fn<(query: string, options: TerminalSearchOptions) => void>();
		const controller = new TerminalFindController({
			clearSearch: vi.fn<() => void>(),
			updateSearch,
		});
		const terminal = terminalTarget({
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			end: { column: 7, row: CanonicalUint64TextSchema.parse('4') },
			preview: 'selected',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('4') },
			state: 'active',
		});
		const registrations = registerProjectedTerminalActions({
			findController: controller,
			scope,
			terminal,
			typedRuntime: createTypedActionRuntime<ProjectedTerminalActionCatalog>(runtime),
		});

		await runtime.execute(PROJECTED_TERMINAL_ACTION_IDS.findOpen, undefined, {
			scope,
			source: 'keyboard',
		}).completion;

		expect(controller.getSnapshot().query).toBe('selected');
		expect(updateSearch).toHaveBeenCalledWith('selected', {
			caseSensitive: false,
			regex: false,
			wholeWord: false,
		});
		for (const registration of registrations) registration.dispose();
		runtime.dispose();
	});

	it('does not rebind an open find query when a pending selection arrives later', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('terminal.projected');
		const updateSearch = vi.fn<(query: string, options: TerminalSearchOptions) => void>();
		const controller = new TerminalFindController({
			clearSearch: vi.fn<() => void>(),
			updateSearch,
		});
		let selection: ProjectedTerminalActionTarget['selection'] = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			state: 'none',
		};
		const target = terminalTarget();
		Object.defineProperty(target, 'selection', { get: () => selection });
		const registrations = registerProjectedTerminalActions({
			findController: controller,
			scope,
			terminal: target,
			typedRuntime: createTypedActionRuntime<ProjectedTerminalActionCatalog>(runtime),
		});

		await runtime.execute(PROJECTED_TERMINAL_ACTION_IDS.selectAll, undefined, {
			scope,
			source: 'keyboard',
		}).completion;
		await runtime.execute(PROJECTED_TERMINAL_ACTION_IDS.findOpen, undefined, {
			scope,
			source: 'keyboard',
		}).completion;
		selection = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			end: { column: 7, row: CanonicalUint64TextSchema.parse('4') },
			preview: 'arrived later',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('4') },
			state: 'active',
		};

		expect(controller.getSnapshot().query).toBe('');
		expect(updateSearch).not.toHaveBeenCalled();
		controller.close();
		await runtime.execute(PROJECTED_TERMINAL_ACTION_IDS.findOpen, undefined, {
			scope,
			source: 'keyboard',
		}).completion;
		expect(controller.getSnapshot().query).toBe('arrived later');
		for (const registration of registrations) registration.dispose();
		runtime.dispose();
	});

	it('discovers the accepted minimal catalog and executes local and clipboard Actions', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('terminal.projected');
		const controller = new TerminalFindController({
			clearSearch: vi.fn<() => void>(),
			updateSearch: vi.fn<(query: string, options: TerminalSearchOptions) => void>(),
		});
		const terminal = terminalTarget();
		vi.mocked(terminal.paste).mockResolvedValue({
			kind: 'sent',
			review: {
				bracketed: false,
				embeddedBracketTerminator: false,
				multiline: false,
				originalText: 'clipboard text',
				replacedControlCharacters: 0,
				source: 'clipboard',
				text: 'clipboard text',
			},
		});
		const registrations = registerProjectedTerminalActions({
			findController: controller,
			platformEffects: {
				readClipboardText: () => ({ kind: 'completed', text: 'clipboard text' }),
			},
			scope,
			terminal,
			typedRuntime: createTypedActionRuntime<ProjectedTerminalActionCatalog>(runtime),
		});

		const discovered = runtime.discover({ scope, source: 'context-menu' });
		expect(
			discovered
				.filter((result) => result.kind === 'ready')
				.map((result) => result.descriptor.id)
				.sort(),
		).toEqual(
			[
				PROJECTED_TERMINAL_ACTION_IDS.clear,
				PROJECTED_TERMINAL_ACTION_IDS.copySelection,
				PROJECTED_TERMINAL_ACTION_IDS.findNext,
				PROJECTED_TERMINAL_ACTION_IDS.findOpen,
				PROJECTED_TERMINAL_ACTION_IDS.findPrevious,
				PROJECTED_TERMINAL_ACTION_IDS.paste,
				PROJECTED_TERMINAL_ACTION_IDS.reset,
				PROJECTED_TERMINAL_ACTION_IDS.scrollToBottom,
				PROJECTED_TERMINAL_ACTION_IDS.selectAll,
			].sort(),
		);
		for (const actionId of [
			PROJECTED_TERMINAL_ACTION_IDS.paste,
			PROJECTED_TERMINAL_ACTION_IDS.scrollToBottom,
			PROJECTED_TERMINAL_ACTION_IDS.clear,
		] as const) {
			await expect(
				runtime.execute(actionId, undefined, { scope, source: 'context-menu' }).completion,
			).resolves.toEqual({ kind: 'completed' });
		}

		expect(terminal.paste).toHaveBeenCalledWith('clipboard text', 'clipboard');
		expect(terminal.scrollToBottom).toHaveBeenCalledOnce();
		expect(terminal.clear).toHaveBeenCalledOnce();
		for (const registration of registrations) registration.dispose();
		runtime.dispose();
	});

	it('rechecks Browser clipboard capability when resolving and executing Copy', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('terminal.projected');
		const controller = new TerminalFindController({
			clearSearch: vi.fn<() => void>(),
			updateSearch: vi.fn<(query: string, options: TerminalSearchOptions) => void>(),
		});
		const terminal = terminalTarget({
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			end: { column: 7, row: CanonicalUint64TextSchema.parse('4') },
			preview: 'selected',
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('4') },
			state: 'active',
		});
		let clipboardWriteAvailable = false;
		const registrations = registerProjectedTerminalActions({
			findController: controller,
			platformEffects: {
				isClipboardWriteAvailable: () => clipboardWriteAvailable,
			},
			scope,
			terminal,
			typedRuntime: createTypedActionRuntime<ProjectedTerminalActionCatalog>(runtime),
		});
		const actionContext = { scope, source: 'keyboard' as const };

		expect(
			runtime.resolveAction(PROJECTED_TERMINAL_ACTION_IDS.copySelection, undefined, actionContext)
				?.availability,
		).toMatchObject({ kind: 'disabled' });
		await expect(
			runtime.execute(PROJECTED_TERMINAL_ACTION_IDS.copySelection, undefined, actionContext)
				.completion,
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
		expect(terminal.copySelectionAsync).not.toHaveBeenCalled();

		clipboardWriteAvailable = true;
		expect(
			runtime.resolveAction(PROJECTED_TERMINAL_ACTION_IDS.copySelection, undefined, actionContext)
				?.availability,
		).toEqual({ kind: 'enabled' });
		await expect(
			runtime.execute(PROJECTED_TERMINAL_ACTION_IDS.copySelection, undefined, actionContext)
				.completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(terminal.copySelectionAsync).toHaveBeenCalledOnce();

		clipboardWriteAvailable = false;
		expect(
			runtime.resolveAction(PROJECTED_TERMINAL_ACTION_IDS.copySelection, undefined, actionContext)
				?.availability,
		).toMatchObject({ kind: 'disabled' });
		for (const registration of registrations) registration.dispose();
		runtime.dispose();
	});

	it('revalidates pinned link context immediately before the external effect', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('terminal.projected');
		const controller = new TerminalFindController({
			clearSearch: vi.fn<() => void>(),
			updateSearch: vi.fn<(query: string, options: TerminalSearchOptions) => void>(),
		});
		const terminal = terminalTarget();
		vi.mocked(terminal.linkOpenRequestIsCurrent).mockReturnValue(false);
		const openExternal = vi.fn<NonNullable<ProjectedTerminalPlatformEffects['openExternal']>>(
			() => ({ kind: 'completed' as const }),
		);
		const registrations = registerProjectedTerminalActions({
			findController: controller,
			platformEffects: { openExternal },
			scope,
			terminal,
			typedRuntime: createTypedActionRuntime<ProjectedTerminalActionCatalog>(runtime),
		});

		await expect(
			runtime.execute(
				PROJECTED_TERMINAL_ACTION_IDS.openLink,
				{
					context: {
						column: 2,
						generation: '1',
						linkEpoch: '3',
						revision: '4',
						row: '5',
					},
					target: { kind: 'url', url: 'https://orchbee.dev/docs' },
				},
				{ scope, source: 'context-menu' },
			).completion,
		).resolves.toEqual({ code: 'stale-terminal-link-context', kind: 'blocked' });
		expect(openExternal).not.toHaveBeenCalled();
		expect(controller.getSnapshot().notice).toBeNull();
		for (const registration of registrations) registration.dispose();
		runtime.dispose();
	});

	it('shares the exact URL allowlist with the external effect boundary', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('terminal.projected');
		const controller = new TerminalFindController({
			clearSearch: vi.fn<() => void>(),
			updateSearch: vi.fn<(query: string, options: TerminalSearchOptions) => void>(),
		});
		const terminal = terminalTarget();
		const openExternal = vi.fn<NonNullable<ProjectedTerminalPlatformEffects['openExternal']>>(
			() => ({ kind: 'completed' as const }),
		);
		const registrations = registerProjectedTerminalActions({
			findController: controller,
			platformEffects: { openExternal },
			scope,
			terminal,
			typedRuntime: createTypedActionRuntime<ProjectedTerminalActionCatalog>(runtime),
		});
		const context = {
			column: 2,
			generation: '1',
			linkEpoch: '3',
			revision: '4',
			row: '5',
		};

		for (const target of [
			'http://orchbee.dev/path',
			'https://orchbee.dev/终端',
			'mailto:terminal@orchbee.dev',
		]) {
			await expect(
				runtime.execute(
					PROJECTED_TERMINAL_ACTION_IDS.openLink,
					{ context, target: { kind: 'url', url: target } },
					{ scope, source: 'context-menu' },
				).completion,
			).resolves.toEqual({ kind: 'completed' });
		}
		for (const target of [
			'file:///tmp/secret',
			'ftp://orchbee.dev',
			'ssh://orchbee.dev',
			'tel:+15551234567',
		]) {
			await expect(
				runtime.execute(
					PROJECTED_TERMINAL_ACTION_IDS.openLink,
					{ context, target: { kind: 'url', url: target } },
					{ scope, source: 'context-menu' },
				).completion,
			).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
		}

		expect(openExternal.mock.calls.map(([target]) => target)).toEqual([
			'http://orchbee.dev/path',
			'https://orchbee.dev/%E7%BB%88%E7%AB%AF',
			'mailto:terminal@orchbee.dev',
		]);
		expect(terminal.linkOpenRequestIsCurrent).toHaveBeenCalledTimes(3);
		for (const registration of registrations) registration.dispose();
		runtime.dispose();
	});
});

function terminalTarget(
	selection: ProjectedTerminalActionTarget['selection'] = {
		attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
		state: 'none',
	},
): ProjectedTerminalActionTarget {
	return {
		canControlTerminal: true,
		clear: vi.fn<ProjectedTerminalActionTarget['clear']>(() => true),
		copySelectionAsync: vi
			.fn<ProjectedTerminalActionTarget['copySelectionAsync']>()
			.mockResolvedValue({ kind: 'completed' }),
		findNext: vi.fn<ProjectedTerminalActionTarget['findNext']>(() => true),
		findPrevious: vi.fn<ProjectedTerminalActionTarget['findPrevious']>(() => true),
		linkOpenRequestIsCurrent: vi.fn<ProjectedTerminalActionTarget['linkOpenRequestIsCurrent']>(
			() => true,
		),
		openLinkAsync: vi
			.fn<ProjectedTerminalActionTarget['openLinkAsync']>()
			.mockResolvedValue({ kind: 'completed' }),
		paste: vi.fn<ProjectedTerminalActionTarget['paste']>().mockResolvedValue({ kind: 'empty' }),
		reset: vi.fn<ProjectedTerminalActionTarget['reset']>(),
		scrollToBottom: vi.fn<ProjectedTerminalActionTarget['scrollToBottom']>(() => true),
		selectAll: vi.fn<ProjectedTerminalActionTarget['selectAll']>(),
		selection,
	};
}
