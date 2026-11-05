import { expect, it, vi } from 'vitest';
import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
	resolveTerminalColorTheme,
	terminalColorThemeAnsi,
} from '@orchbee/terminal';
import {
	createServerTerminalColorsSource,
	presentServerTerminalColors,
} from '../server-terminal-colors';

it('reports the effective palette under fixed appearance and custom defaults and recolors locally', () => {
	let darkTheme = 'gruvbox-dark';
	let terminalDisplayTheme: 'theme' | 'custom' = 'theme';
	const source = createServerTerminalColorsSource(
		{
			getSnapshot: () => ({
				preferences: {
					applicationTheme: 'dark',
					darkTheme,
					lightTheme: 'orchbee-light',
					terminalDisplayTheme,
					customBackground: '#010203',
					customForeground: '#abcdef',
				},
			}),
			subscribeSnapshot: () => () => {},
		},
		{
			matches: false,
			addEventListener: vi.fn<MediaQueryList['addEventListener']>(),
			removeEventListener: vi.fn<MediaQueryList['removeEventListener']>(),
		},
	);
	const theme = resolveTerminalColorTheme('dark', darkTheme);
	expect(source.current().host).toEqual({
		foreground: theme.foreground,
		background: theme.background,
		cursor: theme.cursor,
		palette: terminalColorThemeAnsi(theme),
	});
	const presentation = presentServerTerminalColors(
		{
			current: () => DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			subscribe: () => () => {},
		},
		source,
	);
	const first = presentation.current();
	expect(first.defaultColorOverride?.ansi).toEqual(terminalColorThemeAnsi(theme));
	expect(first.defaultColorOverride?.cursorAccent).toBe(
		`#${theme.cursorText.toString(16).padStart(6, '0')}`,
	);
	darkTheme = 'dracula';
	expect(presentation.current().defaultColorOverride?.ansi).not.toEqual(
		first.defaultColorOverride?.ansi,
	);
	terminalDisplayTheme = 'custom';
	expect(source.current().host).toMatchObject({ foreground: 0xabcdef, background: 0x010203 });
	expect(source.current().host?.palette).toEqual(
		terminalColorThemeAnsi(resolveTerminalColorTheme('dark', darkTheme)),
	);
});
