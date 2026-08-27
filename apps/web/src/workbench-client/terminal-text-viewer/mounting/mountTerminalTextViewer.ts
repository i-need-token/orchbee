import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import { TerminalTextViewer } from '../TerminalTextViewer';
import type {
	TerminalTextViewerController,
	TerminalTextViewerSource,
} from '../model/terminal-text-viewer-controller';

export interface MountTerminalTextViewerOptions {
	readonly controller: TerminalTextViewerController;
	readonly locale: OrchbeeLocale;
	readonly onCopy: (source: TerminalTextViewerSource) => void;
	readonly platform: KeyboardShortcutPlatform;
	readonly restoreFocus: () => void;
	readonly root: HTMLElement;
}

export interface TerminalTextViewerMount {
	dispose(): void;
}

export function mountTerminalTextViewer({
	controller,
	locale,
	onCopy,
	platform,
	restoreFocus,
	root,
}: MountTerminalTextViewerOptions): TerminalTextViewerMount {
	const reactRoot = createRoot(root);
	reactRoot.render(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(
				TooltipProvider,
				undefined,
				createElement(TerminalTextViewer, {
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
