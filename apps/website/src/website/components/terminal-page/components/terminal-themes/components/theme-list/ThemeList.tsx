import { useOrchbeeIntl, websiteTerminalMessages as messages } from '@orchbee/i18n';
import { useId, type ReactElement } from 'react';

import { terminalThemeData } from '../../../../../terminal-text/terminalThemeData';
import {
	DEFAULT_TERMINAL_THEME,
	TERMINAL_THEME_COUNT,
	TERMINAL_THEMES_BY_POLARITY,
	type NamedTerminalTheme,
} from '../../terminal-themes';
import styles from './ThemeList.module.css';

const HEADINGS = { dark: messages.themesDark, light: messages.themesLight } as const;

function hex(color: number): string {
	return `#${color.toString(16).padStart(6, '0')}`;
}

function ThemeButton({ entry }: { entry: NamedTerminalTheme }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const { theme } = entry;
	const data = terminalThemeData(theme);
	// The swatch halves are the theme's own background and foreground, drawn at build time, so the
	// list shows them without script.
	return (
		<button
			className={styles['option']}
			type='button'
			data-terminal-theme={data.polarity}
			data-terminal-colors={data.colors}
			aria-pressed={theme.id === DEFAULT_TERMINAL_THEME.theme.id}
			disabled
		>
			<span className={styles['swatch']}>
				<svg viewBox='0 0 12 12' aria-hidden='true'>
					<path d='M0 0H12L0 12Z' fill={hex(theme.background)} />
					<path d='M12 0V12H0Z' fill={hex(theme.foreground)} />
				</svg>
			</span>
			<span data-theme-label=''>{t(entry.name)}</span>
		</button>
	);
}

/**
 * The product's terminal themes, dark ones then light ones, under a line that counts them. Each is
 * a button that gives the theme window its colors; without script the buttons stay disabled and
 * the window shows the default theme, which is the pressed one. Its root is `data-theme-list`.
 */
export function ThemeList(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const id = useId();
	return (
		<div data-theme-list=''>
			<p className={styles['count']}>{t(messages.themesCount, { count: TERMINAL_THEME_COUNT })}</p>
			{TERMINAL_THEMES_BY_POLARITY.map(({ polarity, themes }) => (
				<div key={polarity}>
					<h3 className={styles['heading']} id={`${id}-${polarity}`}>
						{t(HEADINGS[polarity])}
					</h3>
					<div className={styles['group']} role='group' aria-labelledby={`${id}-${polarity}`}>
						{themes.map((entry) => (
							<ThemeButton key={entry.theme.id} entry={entry} />
						))}
					</div>
				</div>
			))}
		</div>
	);
}
