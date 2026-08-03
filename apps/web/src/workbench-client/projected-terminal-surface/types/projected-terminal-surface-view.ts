import type { RefObject } from 'react';

export interface ProjectedTerminalSurfaceView {
	readonly failureMessage: string | null;
	readonly projectionState: 'failed' | 'inactive' | 'requested';
	readonly root: RefObject<HTMLDivElement | null>;
	readonly stopped: boolean;
	startNewSession(): void;
}
