export interface ProjectedTerminalClipboardActionCoordinator {
	runPaste<Result>(operation: () => Promise<Result>): Promise<Result>;
	shouldInvokePasteShortcut(nativePasteFallback: boolean): boolean;
	trackSelectionCopy<Result>(operation: Promise<Result>): Promise<Result>;
}

export function createProjectedTerminalClipboardActionCoordinator(): ProjectedTerminalClipboardActionCoordinator {
	let activePasteOperations = 0;
	let pendingSelectionCopy: Promise<unknown> | undefined;

	return {
		runPaste: async (operation) => {
			activePasteOperations += 1;
			try {
				const copy = pendingSelectionCopy;
				if (copy !== undefined) {
					try {
						await copy;
					} catch {
						// Copy reports its own failure; Paste still reads the current clipboard.
					}
				}
				return await operation();
			} finally {
				activePasteOperations -= 1;
			}
		},
		shouldInvokePasteShortcut: (nativePasteFallback) =>
			!nativePasteFallback || pendingSelectionCopy !== undefined || activePasteOperations > 0,
		trackSelectionCopy: (operation) => {
			pendingSelectionCopy = operation;
			return operation.finally(() => {
				if (pendingSelectionCopy === operation) pendingSelectionCopy = undefined;
			});
		},
	};
}
