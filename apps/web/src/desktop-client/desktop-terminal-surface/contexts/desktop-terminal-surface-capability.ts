import type { DesktopShortcutPlatform } from '../../terminal-actions';
import type { DesktopTerminalProjectionClient } from '../../terminal-projection-control';
import type { TerminalPresentationId } from '@orchbee/workbench';
import { createContext } from 'react';

export interface DesktopTerminalSurfaceCapability {
	readonly documentElement: Pick<HTMLElement, 'dir' | 'lang'>;
	readonly preferredLocales: readonly string[];
	readonly projectionClient: Pick<DesktopTerminalProjectionClient, 'open'>;
	readonly shortcutPlatform: DesktopShortcutPlatform;
	startStoppedTerminalSession(presentationId: TerminalPresentationId): void;
}

export const DesktopTerminalSurfaceCapabilityContext =
	createContext<DesktopTerminalSurfaceCapability | null>(null);
