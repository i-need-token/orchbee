import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { createElement, type ReactNode } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { KeyboardShortcutSettingsController } from '../../../keyboard-shortcuts';
import { ApplicationLocaleController } from '../../../application-locale';
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

		expect(renderView(controller, 'en-US')).toBe('Settings|12|custom-enabled');
		expect(renderView(controller, 'zh-CN')).toBe('设置|12|custom-enabled');
	});
});

function SurfaceProbe({
	applicationLocaleController,
	controller,
	keyboardShortcutController,
}: {
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly controller: TerminalSettingsController;
	readonly keyboardShortcutController: KeyboardShortcutSettingsController;
}): ReactNode {
	const view = useTerminalSettingsSurface({
		applicationLocaleController,
		controller,
		keyboardShortcutController,
		keyboardShortcutPlatform: 'apple',
		onRequestClose: () => undefined,
	});
	const categoryCount = view.navigationGroups.reduce(
		(total, group) => total + group.items.length,
		0,
	);
	return `${view.messages.title}|${categoryCount}|${view.customColorsEnabled ? 'custom-enabled' : 'custom-disabled'}`;
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
	const keyboardShortcutController = new KeyboardShortcutSettingsController({
		platform: 'apple',
		storage: { getItem: () => null, setItem: () => undefined },
	});
	const applicationLocaleController = new ApplicationLocaleController({
		documentElement: { dir: '', lang: '' },
		preferredLocales: [locale],
		storage: { getItem: () => null, setItem: () => undefined },
	});
	return renderToStaticMarkup(
		createElement(
			OrchbeeI18nProvider,
			{ locale },
			createElement(SurfaceProbe, {
				applicationLocaleController,
				controller,
				keyboardShortcutController,
			}),
		),
	);
}
