import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import { useTerminalSettings, type TerminalSettingsController } from '../../../terminal-settings';
import { useSetting } from '../../hooks/useSetting';
import { useSystemPrefersDark } from '../../hooks/useSystemPrefersDark';
import type { TerminalColorThemeListView } from '../../types/terminal-settings-surface';
import { buildColorThemePickerViews } from '../../utils/buildColorThemePickerViews';
import { matchesSettingsQuery } from '../../utils/matchesSettingsQuery';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SETTINGS_LIST_FILTER_THRESHOLD } from '../../constants/settings-list-filter';
import { SettingsListFilter } from '../settings-list-filter/SettingsListFilter';
import { SettingsListFilterEmpty } from '../settings-list-filter-empty/SettingsListFilterEmpty';
import styles from './ThemeSettings.module.css';

interface ThemeSettingsProps {
	readonly controller: TerminalSettingsController;
	readonly query: string;
}

/** The light and dark theme lists; the list the app appearance shows now is tagged in use. */
export function ThemeSettings({ controller, query }: ThemeSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = useTerminalSettings(controller);
	const systemDark = useSystemPrefersDark();
	const light = useSetting(controller.userSettings, 'appearance.light_theme');
	const dark = useSetting(controller.userSettings, 'appearance.dark_theme');
	const readOnly = light.readOnly;
	const lists = buildColorThemePickerViews({
		colorThemes: controller.colorThemes,
		formatMessage: (descriptor) => intl.formatMessage(descriptor),
		preferences: snapshot.preferences,
		systemDark,
		onCommit: (polarity, themeId) => {
			if (!readOnly) controller.selectColorTheme(polarity, themeId);
		},
		onPreview: (polarity, themeId) =>
			controller.previewColorTheme(themeId === null ? null : { polarity, themeId }),
	});
	return lists.map((list) => {
		const slot = list.polarity === 'dark' ? dark : light;
		return (
			<SettingsGroup
				description={slot.failureMessage ?? list.note}
				key={list.settingId}
				label={list.label}
				modified={slot.modified}
				tag={list.inUse ? intl.formatMessage(terminalSettingsMessages.pageThemesInUse) : null}
				onRestore={readOnly ? undefined : slot.restore}
			>
				<ColorThemeList list={list} query={query} />
			</SettingsGroup>
		);
	});
}

const SWATCH_CELL = { gap: 2, height: 10, insetBlock: 3, insetInline: 4, width: 7 } as const;

/** A strip of the theme's background holding its foreground and five ANSI colors. */
function ThemeSwatch({
	swatch,
}: {
	readonly swatch: TerminalColorThemeListView['options'][number]['swatch'];
}): React.ReactNode {
	const [background, ...cells] = swatch;
	const width =
		SWATCH_CELL.insetInline * 2 +
		cells.length * SWATCH_CELL.width +
		(cells.length - 1) * SWATCH_CELL.gap;
	const height = SWATCH_CELL.insetBlock * 2 + SWATCH_CELL.height;
	return (
		<svg aria-hidden='true' className={styles['swatch']} viewBox={`0 0 ${width} ${height}`}>
			<rect fill={background?.color} height={height} rx={4} width={width} />
			{cells.map((cell, index) => (
				<rect
					fill={cell.color}
					height={SWATCH_CELL.height}
					key={cell.role}
					rx={2}
					width={SWATCH_CELL.width}
					x={SWATCH_CELL.insetInline + index * (SWATCH_CELL.width + SWATCH_CELL.gap)}
					y={SWATCH_CELL.insetBlock}
				/>
			))}
		</svg>
	);
}

interface ColorThemeListProps {
	readonly list: TerminalColorThemeListView;
	readonly query: string;
}

/**
 * One list's themes, one per line. Moving through the list previews the highlighted theme; Enter,
 * Space, or a click commits it; Escape or leaving the list restores the committed theme.
 */
function ColorThemeList({ list, query }: ColorThemeListProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const { label, selectedId, settingId, onCommit, onPreview } = list;
	const [activeId, setActiveId] = React.useState<string | null>(null);
	const [filter, setFilter] = React.useState('');
	const searching = query.trim() !== '';
	const filterVisible = !searching && list.options.length > SETTINGS_LIST_FILTER_THRESHOLD;
	const options = list.options.filter((option) => {
		if (searching) {
			return (
				matchesSettingsQuery(query, [option.name], intl.locale) ||
				(option.id === selectedId && matchesSettingsQuery(query, [label], intl.locale))
			);
		}
		return !filterVisible || matchesSettingsQuery(filter, [option.name], intl.locale);
	});
	const highlightedId = activeId ?? selectedId;
	const optionElementId = (themeId: string): string => `${settingId}-option-${themeId}`;

	const restore = (): void => {
		setActiveId(null);
		onPreview(null);
	};
	const commit = (themeId: string): void => {
		setActiveId(null);
		if (themeId === selectedId) onPreview(null);
		else onCommit(themeId);
	};
	const highlight = (index: number): void => {
		const option = options[Math.min(Math.max(index, 0), options.length - 1)];
		if (option === undefined) return;
		setActiveId(option.id);
		onPreview(option.id === selectedId ? null : option.id);
		document.getElementById(optionElementId(option.id))?.scrollIntoView({ block: 'nearest' });
	};
	const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
		const index = options.findIndex((option) => option.id === highlightedId);
		switch (event.key) {
			case 'ArrowDown':
				event.preventDefault();
				highlight(index + 1);
				return;
			case 'ArrowUp':
				event.preventDefault();
				highlight(index - 1);
				return;
			case 'Home':
				event.preventDefault();
				highlight(0);
				return;
			case 'End':
				event.preventDefault();
				highlight(options.length - 1);
				return;
			case 'Enter':
			case ' ':
				event.preventDefault();
				commit(highlightedId);
				return;
			case 'Escape':
				// Without a preview to restore, Escape belongs to Settings.
				if (activeId === null || activeId === selectedId) return;
				event.preventDefault();
				restore();
		}
	};
	const highlightedVisible = options.some((option) => option.id === highlightedId);

	return (
		<>
			{filterVisible ? (
				<SettingsListFilter
					placeholder={intl.formatMessage(
						list.polarity === 'dark'
							? terminalSettingsMessages.pageFilterDarkThemes
							: terminalSettingsMessages.pageFilterLightThemes,
					)}
					shown={options.length}
					total={list.options.length}
					value={filter}
					onChange={setFilter}
				/>
			) : null}
			{filterVisible && options.length === 0 ? <SettingsListFilterEmpty /> : null}
			{options.length === 0 ? null : (
				<div
					aria-activedescendant={highlightedVisible ? optionElementId(highlightedId) : undefined}
					aria-label={label}
					className={styles['list']}
					id={settingId}
					role='listbox'
					tabIndex={0}
					onBlur={() => {
						if (activeId !== null) restore();
					}}
					onKeyDown={onKeyDown}
				>
					{options.map((option) => {
						const selected = option.id === selectedId;
						return (
							<div
								aria-selected={selected}
								className={styles['option']}
								data-highlighted={option.id === highlightedId}
								data-settings-row
								id={optionElementId(option.id)}
								key={option.id}
								role='option'
								onClick={() => commit(option.id)}
							>
								<ThemeSwatch swatch={option.swatch} />
								<span className={styles['name']}>
									{searching ? `${label}: ${option.name}` : option.name}
								</span>
								<span
									className={styles['check']}
									title={
										selected ? intl.formatMessage(terminalSettingsMessages.pageSelected) : undefined
									}
								>
									{selected ? <ProductIcon name='check' /> : null}
								</span>
							</div>
						);
					})}
				</div>
			)}
		</>
	);
}
