import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import { TerminalFindSurface } from '../TerminalFindSurface';
import type { DesktopTerminalFindController } from '../model/DesktopTerminalFindController';

export interface MountDesktopTerminalFindSurfaceOptions {
	readonly controller: DesktopTerminalFindController;
	readonly locale: OrchbeeLocale;
	readonly onNext: () => void;
	readonly onPrevious: () => void;
	readonly restoreFocus: () => void;
	readonly root: HTMLElement;
}

export interface DesktopTerminalFindSurfaceMount {
	dispose(): void;
}

export function mountDesktopTerminalFindSurface({
	controller,
	locale,
	onNext,
	onPrevious,
	restoreFocus,
	root,
}: MountDesktopTerminalFindSurfaceOptions): DesktopTerminalFindSurfaceMount {
	const reactRoot = createRoot(root);
	reactRoot.render(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(TerminalFindSurface, {
				controller,
				onNext,
				onPrevious,
				restoreFocus,
			}),
		),
	);
	return { dispose: () => reactRoot.unmount() };
}
