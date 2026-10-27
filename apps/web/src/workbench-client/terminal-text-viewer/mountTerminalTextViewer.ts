import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import type { ApplicationLocaleSource } from '../../application-locale';
import { ApplicationLocaleProvider } from '../../application-locale-provider';
import { TerminalTextViewer } from './components/terminal-text-viewer/TerminalTextViewer';
import type {
	TerminalTextViewerController,
	TerminalTextViewerSource,
} from './terminal-text-viewer-controller';

export interface MountTerminalTextViewerOptions {
	readonly applicationLocaleSource?: ApplicationLocaleSource | undefined;
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
	applicationLocaleSource,
	controller,
	locale,
	onCopy,
	platform,
	restoreFocus,
	root,
}: MountTerminalTextViewerOptions): TerminalTextViewerMount {
	const reactRoot = createRoot(root);
	const viewer = createElement(
		TooltipProvider,
		undefined,
		createElement(TerminalTextViewer, {
			controller,
			onCopy,
			platform,
			restoreFocus,
		}),
	);
	reactRoot.render(
		applicationLocaleSource === undefined
			? createElement(OrchbeeI18nProvider, { locale }, viewer)
			: createElement(ApplicationLocaleProvider, { source: applicationLocaleSource }, viewer),
	);
	return { dispose: () => reactRoot.unmount() };
}
