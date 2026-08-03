import { OrchbeeI18nProvider } from '@orchbee/i18n';
import type { ProjectedTerminalSearchEvent } from '@orchbee/terminal';
import { createElement, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';

import {
	TerminalFindController,
	type TerminalSearchDriver,
} from '../../model/TerminalFindController';
import { useTerminalFindSurface } from '../useTerminalFindSurface';

describe('useTerminalFindSurface', () => {
	it('distinguishes invalid queries from unavailable retained history in product copy', () => {
		const invalidController = controllerWithError('invalid-query');
		const unavailableController = controllerWithError('unavailable');

		expect(renderStatus(invalidController)).toBe('Enter a valid regular expression.');
		expect(renderStatus(unavailableController)).toBe(
			'Retained terminal output could not be searched.',
		);
	});
});

function StatusProbe({ controller }: { readonly controller: TerminalFindController }): ReactNode {
	const view = useTerminalFindSurface({
		controller,
		onNext: vi.fn<() => void>(),
		onPrevious: vi.fn<() => void>(),
		restoreFocus: vi.fn<() => void>(),
	});
	return view.statusText;
}

function controllerWithError(
	errorKind: Exclude<ProjectedTerminalSearchEvent['errorKind'], null>,
): TerminalFindController {
	const controller = new TerminalFindController({
		clearSearch: vi.fn<TerminalSearchDriver['clearSearch']>(),
		updateSearch: vi.fn<TerminalSearchDriver['updateSearch']>(),
	});
	controller.setQuery('query');
	controller.applySearchEvent({
		activeIndex: -1,
		errorKind,
		generation: 1n,
		matches: [],
		query: 'query',
		regexError: 'failed',
		resultLimitReached: false,
		revision: 1n,
		state: 'error',
	});
	return controller;
}

function renderStatus(controller: TerminalFindController): string {
	return renderToStaticMarkup(
		createElement(
			OrchbeeI18nProvider,
			{ locale: 'en-US' },
			createElement(StatusProbe, { controller }),
		),
	);
}
