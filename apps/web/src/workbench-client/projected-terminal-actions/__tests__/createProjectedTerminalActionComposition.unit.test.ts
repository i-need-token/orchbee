import { ActionRuntime } from '@orchbee/actions';
import { AttachmentIdSchema, CanonicalUint64TextSchema } from '@orchbee/terminal-protocol';
import type { TerminalSearchOptions } from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import { TerminalFindController } from '../../terminal-find-surface';
import type { ProjectedTerminalPlatformEffects } from '../../projected-terminal-platform-effects';
import {
	createProjectedTerminalActionComposition,
	type ProjectedTerminalActionResultEvent,
} from '../createProjectedTerminalActionComposition';
import { PROJECTED_TERMINAL_ACTION_IDS } from '../projected-terminal-action-ids';
import type { ProjectedTerminalActionTarget } from '../registerProjectedTerminalActions';

describe('createProjectedTerminalActionComposition', () => {
	it('commits clipboard success only after the Main-owned effect completes', async () => {
		const terminal = terminalTarget();
		const controller = findController();
		const { composition, runtime } = createComposition(terminal, controller);
		const execution = composition.execute(
			PROJECTED_TERMINAL_ACTION_IDS.copySelection,
			undefined,
			composition.terminalScope,
			'keyboard',
		);

		await expect(execution.completion).resolves.toEqual({ kind: 'completed' });
		expect(terminal.copySelectionAsync).toHaveBeenCalledOnce();
		expect(controller.getSnapshot().notice).toBeNull();
		composition.dispose();
		runtime.dispose();
	});

	it('exposes blocked file targets as disabled link Actions', async () => {
		const terminal = terminalTarget();
		const controller = findController();
		const { composition, runtime } = createComposition(terminal, controller);
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
		runtime.dispose();
	});

	it('declines a shortcut while its Action is unavailable', () => {
		const terminal = terminalTarget('none');
		const controller = findController();
		const { composition, runtime } = createComposition(terminal, controller);

		expect(
			composition.invokeShortcut(
				PROJECTED_TERMINAL_ACTION_IDS.copySelection,
				undefined,
				composition.terminalScope,
			),
		).toBe(false);
		expect(terminal.copySelectionAsync).not.toHaveBeenCalled();
		composition.dispose();
		runtime.dispose();
	});

	it('leaves an idle platform-default Paste shortcut to native clipboard delivery', () => {
		const terminal = terminalTarget();
		const controller = findController();
		const readClipboardText = vi.fn<
			NonNullable<ProjectedTerminalPlatformEffects['readClipboardText']>
		>(() => ({ kind: 'completed', text: '' }));
		const { composition, runtime } = createComposition(terminal, controller, {
			platformEffects: { readClipboardText },
		});

		expect(
			composition.invokeShortcut(
				PROJECTED_TERMINAL_ACTION_IDS.paste,
				undefined,
				composition.terminalScope,
				{ nativePasteFallback: true },
			),
		).toBe(false);
		expect(readClipboardText).not.toHaveBeenCalled();
		expect(terminal.paste).not.toHaveBeenCalled();
		composition.dispose();
		runtime.dispose();
	});

	it('re-resolves live input capability while local output Actions remain available', () => {
		let canControlTerminal = true;
		const terminal = terminalTarget();
		Object.defineProperty(terminal, 'canControlTerminal', {
			get: () => canControlTerminal,
		});
		const controller = findController();
		const { composition, runtime } = createComposition(terminal, controller);
		canControlTerminal = false;

		expect(
			composition.invokeShortcut(
				PROJECTED_TERMINAL_ACTION_IDS.reset,
				undefined,
				composition.terminalScope,
			),
		).toBe(false);
		expect(
			composition.invokeShortcut(
				PROJECTED_TERMINAL_ACTION_IDS.findOpen,
				undefined,
				composition.terminalScope,
			),
		).toBe(true);
		expect(
			composition.invokeShortcut(
				PROJECTED_TERMINAL_ACTION_IDS.selectAll,
				undefined,
				composition.terminalScope,
			),
		).toBe(true);
		expect(
			composition.invokeShortcut(
				PROJECTED_TERMINAL_ACTION_IDS.copySelection,
				undefined,
				composition.terminalScope,
			),
		).toBe(true);
		expect(terminal.reset).not.toHaveBeenCalled();
		expect(terminal.selectAll).toHaveBeenCalledOnce();
		expect(terminal.copySelectionAsync).toHaveBeenCalledOnce();
		expect(controller.getSnapshot().isOpen).toBe(true);
		composition.dispose();
		runtime.dispose();
	});

	it('cancels in-flight work and suppresses late feedback when the surface is disposed', async () => {
		let resolveClipboardRead:
			| ((result: { readonly kind: 'completed'; readonly text: string }) => void)
			| undefined;
		const readClipboardText = vi.fn<
			NonNullable<ProjectedTerminalPlatformEffects['readClipboardText']>
		>(
			() =>
				new Promise((resolve) => {
					resolveClipboardRead = resolve;
				}),
		);
		const reportResult = vi.fn<(event: ProjectedTerminalActionResultEvent) => void>();
		const terminal = terminalTarget();
		const controller = findController();
		const { composition, runtime } = createComposition(terminal, controller, {
			platformEffects: { readClipboardText },
			reportResult,
		});
		const execution = composition.execute(
			PROJECTED_TERMINAL_ACTION_IDS.paste,
			undefined,
			composition.terminalScope,
			'context-menu',
		);
		await vi.waitFor(() => expect(readClipboardText).toHaveBeenCalledOnce());

		composition.dispose();
		resolveClipboardRead?.({ kind: 'completed', text: 'late clipboard text' });

		await expect(execution.completion).resolves.toEqual({ kind: 'cancelled' });
		expect(terminal.paste).not.toHaveBeenCalled();
		expect(reportResult).not.toHaveBeenCalled();
		runtime.dispose();
	});
});

function createComposition(
	terminal: ProjectedTerminalActionTarget,
	controller: TerminalFindController,
	options: Pick<
		Parameters<typeof createProjectedTerminalActionComposition>[2],
		'platformEffects' | 'reportResult'
	> = {},
) {
	const runtime = new ActionRuntime();
	const terminalScope = runtime.createScope('test.terminal');
	return {
		composition: createProjectedTerminalActionComposition(terminal, controller, {
			...options,
			runtime,
			terminalScope,
		}),
		runtime,
	};
}

function findController(): TerminalFindController {
	return new TerminalFindController({
		clearSearch: vi.fn<() => void>(),
		updateSearch: vi.fn<(query: string, options: TerminalSearchOptions) => void>(),
	});
}

function terminalTarget(
	selectionState: ProjectedTerminalActionTarget['selection']['state'] = 'active',
): ProjectedTerminalActionTarget {
	return {
		canControlTerminal: true,
		canNavigatePrompts: true,
		canScrollHistory: true,
		clear: vi.fn<ProjectedTerminalActionTarget['clear']>(() => true),
		copySelectionAsync: vi
			.fn<ProjectedTerminalActionTarget['copySelectionAsync']>()
			.mockResolvedValue({ byteLength: 8, kind: 'completed' }),
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
		scrollLines: vi.fn<ProjectedTerminalActionTarget['scrollLines']>(() => true),
		scrollPages: vi.fn<ProjectedTerminalActionTarget['scrollPages']>(() => true),
		scrollToNextPrompt: vi
			.fn<ProjectedTerminalActionTarget['scrollToNextPrompt']>()
			.mockResolvedValue(true),
		scrollToBottom: vi.fn<ProjectedTerminalActionTarget['scrollToBottom']>(() => true),
		scrollToPreviousPrompt: vi
			.fn<ProjectedTerminalActionTarget['scrollToPreviousPrompt']>()
			.mockResolvedValue(true),
		scrollToTop: vi.fn<ProjectedTerminalActionTarget['scrollToTop']>(() => true),
		selectAll: vi.fn<ProjectedTerminalActionTarget['selectAll']>(),
		selection: {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			end: { column: 1, row: CanonicalUint64TextSchema.parse('1') },
			preview: null,
			rectangle: false,
			start: { column: 0, row: CanonicalUint64TextSchema.parse('1') },
			state: selectionState,
		},
	};
}
