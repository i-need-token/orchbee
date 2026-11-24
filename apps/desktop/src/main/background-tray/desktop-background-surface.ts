import type { DesktopBackgroundState } from './desktop-background-lifecycle';

/** What the person can ask for from the platform's background surface. */
export interface DesktopBackgroundSurfaceActions {
	/** The one-time notice reached the person, so later runs need not show it again. */
	readonly onNoticeShown: () => void;
	/** Quit Orchbee, which keeps the terminals running. */
	readonly onQuit: () => void;
	/** Stop Session…, which the code calls Quit All. */
	readonly onQuitAll: () => void;
	/** Open Orchbee: shows and focuses the window, or opens one. */
	readonly onReopen: () => void;
}

/**
 * Where Desktop shows that it runs, with or without a window: the icon in the notification area on
 * Windows and in the menu bar on macOS. It projects each lifecycle state itself and reports what the
 * person asks for through its actions.
 */
export interface DesktopBackgroundSurface {
	dispose(): void;
	/** Shows the one-time notice that Orchbee keeps running, naming how many terminals do. */
	showNotice(runningTerminals: number): void;
	update(state: DesktopBackgroundState): void;
}
