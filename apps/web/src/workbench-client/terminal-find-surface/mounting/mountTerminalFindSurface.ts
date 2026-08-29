import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import { TooltipProvider } from '@orchbee/ui/components';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

import type { ApplicationLocaleSource } from '../../../application-locale';
import { ApplicationLocaleProvider } from '../../../application-locale-provider';
import { TerminalFindSurface } from '../TerminalFindSurface';
import type { TerminalFindController } from '../model/TerminalFindController';

export interface MountTerminalFindSurfaceOptions {
	readonly applicationLocaleSource?: ApplicationLocaleSource | undefined;
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
	applicationLocaleSource,
	controller,
	locale,
	onNext,
	onPrevious,
	restoreFocus,
	root,
}: MountTerminalFindSurfaceOptions): TerminalFindSurfaceMount {
	const reactRoot = createRoot(root);
	const surface = createElement(
		TooltipProvider,
		undefined,
		createElement(TerminalFindSurface, {
			controller,
			onNext,
			onPrevious,
			restoreFocus,
		}),
	);
	reactRoot.render(
		applicationLocaleSource === undefined
			? createElement(OrchbeeI18nProvider, { locale }, surface)
			: createElement(ApplicationLocaleProvider, { source: applicationLocaleSource }, surface),
	);
	return { dispose: () => reactRoot.unmount() };
}
