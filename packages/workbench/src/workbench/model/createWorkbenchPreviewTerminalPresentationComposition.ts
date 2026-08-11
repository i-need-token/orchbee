import {
	type TerminalPresentationCommand,
	type TerminalPresentationId,
	type TerminalPresentationOperationId,
	type TerminalPresentationRemovalId,
} from '../../terminal-presentation';

export type WorkbenchTerminalPresentationCommand =
	| TerminalPresentationCommand
	| {
			readonly removalId: TerminalPresentationRemovalId;
			readonly tabId: string;
			readonly type: 'tab-close-ready';
	  };

interface WorkbenchTerminalPresentationIdentityComposition {
	readonly createPresentationId: () => TerminalPresentationId;
	readonly initialPresentationId: TerminalPresentationId;
}

export type WorkbenchTerminalPresentationComposition =
	| (WorkbenchTerminalPresentationIdentityComposition & {
			readonly kind: 'preview';
	  })
	| (WorkbenchTerminalPresentationIdentityComposition & {
			readonly createOperationId: () => TerminalPresentationOperationId;
			readonly createRemovalId: () => TerminalPresentationRemovalId;
			readonly dispatchCommands: (
				commands: readonly WorkbenchTerminalPresentationCommand[],
			) => void;
			readonly kind: 'managed';
	  });

export function createWorkbenchPreviewTerminalPresentationComposition(
	initialPresentationId: TerminalPresentationId,
	createPresentationId: () => TerminalPresentationId,
): WorkbenchTerminalPresentationComposition {
	return {
		createPresentationId,
		initialPresentationId,
		kind: 'preview',
	};
}
