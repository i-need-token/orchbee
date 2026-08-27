import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import { TerminalCommandOutputViewer } from '../TerminalCommandOutputViewer';
import type { TerminalCommandOutputViewerController } from '../model/terminal-command-output-viewer-controller';

export interface MountTerminalCommandOutputViewerOptions {
	readonly controller: TerminalCommandOutputViewerController;
	readonly locale: OrchbeeLocale;
	readonly onCopy: () => void;
	readonly platform: KeyboardShortcutPlatform;
	readonly restoreFocus: () => void;
	readonly root: HTMLElement;
}

export interface TerminalCommandOutputViewerMount {
	dispose(): void;
}

export function mountTerminalCommandOutputViewer({
	controller,
	locale,
	onCopy,
	platform,
	restoreFocus,
	root,
}: MountTerminalCommandOutputViewerOptions): TerminalCommandOutputViewerMount {
	const reactRoot = createRoot(root);
	reactRoot.render(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(
				TooltipProvider,
				undefined,
				createElement(TerminalCommandOutputViewer, {
					controller,
					onCopy,
					platform,
					restoreFocus,
				}),
			),
		),
	);
	return { dispose: () => reactRoot.unmount() };
}
