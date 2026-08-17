import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { createElement, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	TerminalSettingsController,
	type TerminalAppearancePreferences,
} from '../../../terminal-settings';
import { useTerminalSettingsSurface } from '../useTerminalSettingsSurface';

describe('useTerminalSettingsSurface', () => {
	it('prepares localized display data and Custom-mode control availability from the controller snapshot', () => {
		const controller = createController({
			...DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
			terminalDisplayTheme: 'custom',
		});
		controller.open();

		expect(renderView(controller, 'en-US')).toBe('Settings|8|custom-enabled');
		expect(renderView(controller, 'zh-CN')).toBe('设置|8|custom-enabled');
	});
});

function SurfaceProbe({
	controller,
}: {
	readonly controller: TerminalSettingsController;
}): ReactNode {
	const view = useTerminalSettingsSurface({ controller, onRequestClose: () => undefined });
	return `${view.messages.title}|${view.terminalResultCount}|${view.customColorsEnabled ? 'custom-enabled' : 'custom-disabled'}`;
}

function createController(preferences: TerminalAppearancePreferences): TerminalSettingsController {
	return new TerminalSettingsController({
		storage: {
			getItem: () => JSON.stringify(preferences),
			setItem: () => undefined,
		},
		themeTarget: { dataset: {} },
	});
}

function renderView(controller: TerminalSettingsController, locale: 'en-US' | 'zh-CN'): string {
	return renderToStaticMarkup(
		createElement(OrchbeeI18nProvider, { locale }, createElement(SurfaceProbe, { controller })),
	);
}
