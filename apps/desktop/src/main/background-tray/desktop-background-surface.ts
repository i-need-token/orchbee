import type { DesktopBackgroundState } from './desktop-background-lifecycle';

/** What the person can ask for from the platform's background surface. */
export interface DesktopBackgroundSurfaceActions {
	/** The one-time notice reached the person, so later runs need not show it again. */
	readonly onNoticeShown: () => void;
	/** Quit Orchbee, Keep Terminals Running. */
	readonly onQuit: () => void;
	readonly onQuitAll: () => void;
	readonly onReopen: () => void;
}

/**
 * Where Desktop shows that it keeps running without a window: the notification-area icon on Windows,
 * the Dock menu on macOS. It projects each lifecycle state itself and reports what the person asks
 * for through its actions.
 */
export interface DesktopBackgroundSurface {
	dispose(): void;
	/** Shows the one-time notice that Orchbee keeps running, naming how many terminals do. */
	showNotice(runningTerminals: number): void;
	update(state: DesktopBackgroundState): void;
}
