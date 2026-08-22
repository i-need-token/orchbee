import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import { TerminalFindSurface } from '../TerminalFindSurface';
import type { TerminalFindController } from '../model/TerminalFindController';

export interface MountTerminalFindSurfaceOptions {
	readonly controller: TerminalFindController;
	readonly locale: OrchbeeLocale;
	readonly onNext: () => void;
	readonly onPrevious: () => void;
	readonly restoreFocus: () => void;
	readonly root: HTMLElement;
}

export interface TerminalFindSurfaceMount {
	dispose(): void;
}

export function mountTerminalFindSurface({
	controller,
	locale,
	onNext,
	onPrevious,
	restoreFocus,
	root,
}: MountTerminalFindSurfaceOptions): TerminalFindSurfaceMount {
	const reactRoot = createRoot(root);
	reactRoot.render(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(
				TooltipProvider,
				undefined,
				createElement(TerminalFindSurface, {
					controller,
					onNext,
					onPrevious,
					restoreFocus,
				}),
			),
		),
	);
	return { dispose: () => reactRoot.unmount() };
}
