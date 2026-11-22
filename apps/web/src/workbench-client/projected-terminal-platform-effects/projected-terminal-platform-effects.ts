import type { ProjectedTerminalSideEffectResult } from '@orchbee/terminal';
import type { WorkbenchTerminalBellEffect } from '../../workbench';

type ProjectedTerminalClipboardReadResult =
	| { readonly kind: 'completed'; readonly text: string }
	| { readonly code: string; readonly kind: 'failed'; readonly retryable: boolean }
	| { readonly kind: 'cancelled' };

export interface ProjectedTerminalPlatformEffects {
	readonly isClipboardWriteAvailable?: () => boolean;
	readonly playBellSound?: (event: WorkbenchTerminalBellEffect) => void;
	readonly showBellNotification?: (event: WorkbenchTerminalBellEffect) => void;
	readonly openExternal?: (
		target: string,
	) => ProjectedTerminalSideEffectResult | Promise<ProjectedTerminalSideEffectResult>;
	/**
	 * The text a drop of files pastes, read after the drop; null when it has none. Only Desktop
	 * supplies it, so elsewhere a terminal takes no file drops.
	 */
	readonly takeDroppedFileText?: () => Promise<string | null>;
	readonly readClipboardText?: () =>
		| ProjectedTerminalClipboardReadResult
		| Promise<ProjectedTerminalClipboardReadResult>;
	readonly writeClipboardText?: (
		text: string,
	) => ProjectedTerminalSideEffectResult | Promise<ProjectedTerminalSideEffectResult>;
}
