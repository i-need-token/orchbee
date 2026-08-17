// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import { useGhosttyMigrationSettings } from '../useGhosttyMigrationSettings';

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

afterEach(() => cleanup());

describe('useGhosttyMigrationSettings', () => {
	it('projects a localized empty dry-run contract without analyzing or retaining source text', () => {
		const view = renderHook(() => useGhosttyMigrationSettings(), { wrapper: I18nWrapper });

		expect(view.result.current).toMatchObject({
			canAnalyze: false,
			canClear: false,
			messages: {
				analyze: 'Analyze configuration',
				safetyNotice: 'This is a dry run. Orchbee will not change or save any settings.',
				sourceLabel: 'Ghostty configuration',
			},
			report: null,
			source: '',
		});
	});
});
