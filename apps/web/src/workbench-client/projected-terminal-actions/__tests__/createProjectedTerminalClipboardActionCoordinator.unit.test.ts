import { describe, expect, it, vi } from 'vitest';

import { createProjectedTerminalClipboardActionCoordinator } from '../createProjectedTerminalClipboardActionCoordinator';

describe('createProjectedTerminalClipboardActionCoordinator', () => {
	it('routes only an idle platform-default Paste shortcut to native delivery', async () => {
		const coordinator = createProjectedTerminalClipboardActionCoordinator();

		expect(coordinator.shouldInvokePasteShortcut(true)).toBe(false);
		expect(coordinator.shouldInvokePasteShortcut(false)).toBe(true);

		let completePaste: (() => void) | undefined;
		const paste = coordinator.runPaste(
			() =>
				new Promise<void>((resolve) => {
					completePaste = resolve;
				}),
		);
		expect(coordinator.shouldInvokePasteShortcut(true)).toBe(true);
		completePaste?.();
		await paste;

		expect(coordinator.shouldInvokePasteShortcut(true)).toBe(false);
	});

	it('waits for the exact selection Copy before Paste and clears either settlement', async () => {
		const coordinator = createProjectedTerminalClipboardActionCoordinator();
		let completeCopy: ((value: string) => void) | undefined;
		const copy = new Promise<string>((resolve) => {
			completeCopy = resolve;
		});
		const trackedCopy = coordinator.trackSelectionCopy(copy);
		const executePaste = vi.fn<() => Promise<string>>(() => Promise.resolve('pasted'));

		const paste = coordinator.runPaste(executePaste);
		await Promise.resolve();
		expect(coordinator.shouldInvokePasteShortcut(true)).toBe(true);
		expect(executePaste).not.toHaveBeenCalled();

		completeCopy?.('copied');
		await expect(trackedCopy).resolves.toBe('copied');
		await expect(paste).resolves.toBe('pasted');
		expect(executePaste).toHaveBeenCalledOnce();
		expect(coordinator.shouldInvokePasteShortcut(true)).toBe(false);

		let rejectCopy: ((reason: Error) => void) | undefined;
		const failedCopy = coordinator.trackSelectionCopy(
			new Promise<never>((_resolve, reject) => {
				rejectCopy = reject;
			}),
		);
		const pasteAfterFailure = coordinator.runPaste(() => Promise.resolve('current clipboard'));
		rejectCopy?.(new Error('copy failed'));
		await expect(failedCopy).rejects.toThrow('copy failed');
		await expect(pasteAfterFailure).resolves.toBe('current clipboard');
		expect(coordinator.shouldInvokePasteShortcut(true)).toBe(false);
	});
});
