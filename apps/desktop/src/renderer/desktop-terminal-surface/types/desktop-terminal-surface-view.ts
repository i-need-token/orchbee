import type { RefObject } from 'react';

export interface DesktopTerminalSurfaceView {
	readonly failureMessage: string | null;
	readonly projectionState: 'failed' | 'inactive' | 'requested';
	readonly root: RefObject<HTMLDivElement | null>;
}
