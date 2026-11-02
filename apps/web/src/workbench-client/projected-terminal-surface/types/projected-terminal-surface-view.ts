import type { DragEvent, RefObject } from 'react';
import type { ProjectedPasteReview, TerminalImagePreviewPresentation } from '@orchbee/terminal';
import type {
	MobileTerminalInputState,
	MobileTerminalKey,
	MobileTerminalModifier,
} from '../../projected-terminal-renderer';

/** A terminal shown without one: its creation failed, or nothing launched it. */
export type ProjectedTerminalLaunchFailureView =
	| { readonly executable: string | null; readonly kind: 'failed' }
	| { readonly kind: 'unavailable' };

export interface ProjectedTerminalSurfaceView {
	readonly failureMessage: string | null;
	readonly imagePreview: Extract<
		TerminalImagePreviewPresentation,
		{ readonly kind: 'visible' }
	> | null;
	readonly launchFailure: ProjectedTerminalLaunchFailureView | null;
	readonly launchFailureRoot: RefObject<HTMLElement | null>;
	readonly mobileInputState: MobileTerminalInputState;
	readonly pasteReview: ProjectedPasteReview | null;
	readonly projectionState: 'failed' | 'inactive' | 'recovering' | 'requested';
	readonly recoveryAvailable: boolean;
	readonly root: RefObject<HTMLDivElement | null>;
	readonly stopped: boolean;
	approvePaste(): void;
	cancelPaste(): void;
	closeTerminal(): void;
	createTab(): void;
	handleFileDragOver(event: DragEvent<HTMLDivElement>): void;
	handleFileDrop(event: DragEvent<HTMLDivElement>): void;
	retryProjection(): void;
	sendMobileKey(key: MobileTerminalKey): boolean;
	startNewSession(): void;
	toggleMobileKeyboard(): boolean;
	toggleMobileModifier(modifier: MobileTerminalModifier): void;
}
