import type { DesktopShortcutPlatform } from '../../terminal-actions';
import type { DesktopTerminalProjectionClient } from '../../terminal-projection-control';
import { createContext } from 'react';

export interface DesktopTerminalSurfaceCapability {
	readonly documentElement: Pick<HTMLElement, 'dir' | 'lang'>;
	readonly preferredLocales: readonly string[];
	readonly projectionClient: Pick<DesktopTerminalProjectionClient, 'open'>;
	readonly shortcutPlatform: DesktopShortcutPlatform;
}

export const DesktopTerminalSurfaceCapabilityContext =
	createContext<DesktopTerminalSurfaceCapability | null>(null);
