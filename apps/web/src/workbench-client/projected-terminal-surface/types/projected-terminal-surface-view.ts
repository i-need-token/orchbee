import type { RefObject } from 'react';
import type { ProjectedPasteReview } from '@orchbee/terminal';
import type { ProjectedTerminalControlPresentation } from '../../projected-terminal-actions';
import type {
	MobileTerminalInputState,
	MobileTerminalKey,
	MobileTerminalModifier,
	ProjectedTerminalOutcome,
} from '../../projected-terminal-renderer';

/**
 * A terminal shown without one: its creation failed, a restore held it because its saved directory
 * was unavailable, or nothing launched it.
 */
export type ProjectedTerminalLaunchFailureView =
	| { readonly executable: string | null; readonly kind: 'failed' }
	| { readonly directory: string; readonly kind: 'directory-unavailable' }
	| { readonly kind: 'unavailable' };

export interface ProjectedTerminalSurfaceView {
	readonly controlPresentation: ProjectedTerminalControlPresentation;
	readonly failureMessage: string | null;
	readonly launchFailure: ProjectedTerminalLaunchFailureView | null;
	readonly launchFailureRoot: RefObject<HTMLElement | null>;
	/** The pane shows only the part of a larger terminal around its cursor. */
	readonly liveCropped: boolean;
	readonly mobileInputState: MobileTerminalInputState;
	readonly pasteReview: ProjectedPasteReview | null;
	readonly projectionState: 'failed' | 'inactive' | 'recovering' | 'requested';
	readonly recoveryAvailable: boolean;
	readonly root: RefObject<HTMLDivElement | null>;
	/** How the shown terminal ended, absent while it runs. */
	readonly terminalOutcome: ProjectedTerminalOutcome | null;
	approvePaste(): void;
	cancelPaste(): void;
	closeTerminal(): void;
	createTab(): void;
	/** Starts a held restore's terminal; null where this surface cannot. */
	readonly restoreTerminal: ((directory: 'home' | 'saved') => Promise<void>) | null;
	retryProjection(): void;
	sendMobileKey(key: MobileTerminalKey): boolean;
	takeControl(): void;
	toggleMobileKeyboard(): boolean;
	toggleMobileModifier(modifier: MobileTerminalModifier): void;
}
