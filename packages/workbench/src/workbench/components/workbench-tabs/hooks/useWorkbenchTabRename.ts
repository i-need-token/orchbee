import * as React from 'react';

interface RenameTarget {
	readonly draftLabel: string;
	readonly originalLabel: string;
	readonly tabId: string;
}

interface WorkbenchTabRenameState {
	readonly cancel: () => void;
	readonly canSubmit: boolean;
	readonly draftLabel: string;
	readonly isOpen: boolean;
	readonly open: (tabId: string, label: string) => void;
	readonly originalLabel: string;
	readonly setDraftLabel: (draftLabel: string) => void;
	readonly submit: () => void;
}

export function useWorkbenchTabRename(
	onTabRename: ((tabId: string, label: string) => void) | undefined,
): WorkbenchTabRenameState {
	const [target, setTarget] = React.useState<RenameTarget>();
	const trimmedLabel = target?.draftLabel.trim() ?? '';

	return {
		canSubmit: target !== undefined && trimmedLabel.length > 0,
		draftLabel: target?.draftLabel ?? '',
		isOpen: target !== undefined,
		originalLabel: target?.originalLabel ?? '',
		cancel: () => {
			setTarget(undefined);
		},
		open: (tabId: string, label: string) => {
			setTarget({ draftLabel: label, originalLabel: label, tabId });
		},
		setDraftLabel: (draftLabel: string) => {
			setTarget((currentTarget) =>
				currentTarget === undefined ? undefined : { ...currentTarget, draftLabel },
			);
		},
		submit: () => {
			if (target === undefined || trimmedLabel.length === 0 || onTabRename === undefined) return;
			onTabRename(target.tabId, trimmedLabel);
			setTarget(undefined);
		},
	};
}
