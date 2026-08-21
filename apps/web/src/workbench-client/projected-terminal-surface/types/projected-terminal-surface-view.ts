import type { DragEvent, RefObject } from 'react';
import type { ProjectedPasteReview } from '@orchbee/terminal';

export interface ProjectedTerminalSurfaceView {
	readonly failureMessage: string | null;
	readonly pasteReview: ProjectedPasteReview | null;
	readonly projectionState: 'failed' | 'inactive' | 'recovering' | 'requested';
	readonly recoveryAvailable: boolean;
	readonly root: RefObject<HTMLDivElement | null>;
	readonly stopped: boolean;
	approvePaste(): void;
	cancelPaste(): void;
	handleFileDragOver(event: DragEvent<HTMLDivElement>): void;
	handleFileDrop(event: DragEvent<HTMLDivElement>): void;
	retryProjection(): void;
	startNewSession(): void;
}
