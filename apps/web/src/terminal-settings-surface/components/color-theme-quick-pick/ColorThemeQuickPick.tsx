import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	Combobox,
	ComboboxContent,
	ComboboxItem,
	ComboboxList,
	ComboboxSearch,
} from '@orchbee/ui/components';
import * as React from 'react';

import { useTerminalSettings, type TerminalSettingsController } from '../../../terminal-settings';
import { useSystemPrefersDark } from '../../hooks/useSystemPrefersDark';
import type {
	TerminalColorThemeListView,
	TerminalColorThemeOptionView,
} from '../../types/terminal-settings-surface';
import { buildColorThemePickerViews } from '../../utils/buildColorThemePickerViews';
import { matchesSettingsQuery } from '../../utils/matchesSettingsQuery';
import { SettingsListFilterEmpty } from '../settings-list-filter-empty/SettingsListFilterEmpty';
import styles from './ColorThemeQuickPick.module.css';

/** Pixels between the window's top edge and the list. */
const TOP_OFFSET = 48;

interface ColorThemeQuickPickProps {
	readonly controller: TerminalSettingsController;
}

/**
 * The Light and Dark theme lists, each standing over the window while it is open. Both stay mounted
 * while closed, so each list has its selection before it opens and its highlight starts on the
 * theme its slot holds. Moving the highlight previews that theme on the whole window; Enter or a
 * click stores it in its polarity's slot; Escape or a press outside drops the preview. Either way
 * Settings reopens at that button.
 */
export function ColorThemeQuickPick({ controller }: ColorThemeQuickPickProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = useTerminalSettings(controller);
	const systemDark = useSystemPrefersDark();
	const lists = React.useMemo(
		() =>
			buildColorThemePickerViews({
				colorThemes: controller.colorThemes,
				formatMessage: (descriptor) => intl.formatMessage(descriptor),
				preferences: snapshot.preferences,
				systemDark,
			}),
		[controller.colorThemes, intl, snapshot.preferences, systemDark],
	);
	return (
		<>
			{lists.map((list) => (
				<ColorThemeList
					controller={controller}
					key={list.polarity}
					list={list}
					open={snapshot.colorThemePicker === list.polarity}
				/>
			))}
		</>
	);
}

interface ColorThemeListProps {
	readonly controller: TerminalSettingsController;
	readonly list: TerminalColorThemeListView;
	readonly open: boolean;
}

/** One polarity's list. Every close clears its search, so the next open shows all of its themes. */
function ColorThemeList({ controller, list, open }: ColorThemeListProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const [query, setQuery] = React.useState('');
	const options = React.useMemo(
		() => list.options.filter((option) => matchesSettingsQuery(query, [option.name], intl.locale)),
		[intl.locale, list.options, query],
	);
	const selected = list.options.find((option) => option.id === list.selectedId) ?? null;
	const anchor = React.useMemo(
		() => ({ getBoundingClientRect: () => new DOMRect(window.innerWidth / 2, 0, 0, 0) }),
		[],
	);
	const preview = (option: TerminalColorThemeOptionView | undefined): void => {
		if (option === undefined) controller.previewColorTheme(null);
		else controller.previewColorTheme({ polarity: list.polarity, themeId: option.id });
	};
	const close = (): void => {
		setQuery('');
		controller.closeColorThemePicker();
	};
	const keep = (option: TerminalColorThemeOptionView): void => {
		if (option.id !== list.selectedId) controller.selectColorTheme(list.polarity, option.id);
		close();
	};
	return (
		<Combobox
			filter={null}
			inputValue={query}
			isItemEqualToValue={(
				item: TerminalColorThemeOptionView,
				current: TerminalColorThemeOptionView,
			) => item.id === current.id}
			items={options}
			itemToStringLabel={(option: TerminalColorThemeOptionView) => option.name}
			open={open}
			value={selected}
			onInputValueChange={setQuery}
			onItemHighlighted={(option, details) => {
				// The highlight Base UI sets as the list opens is programmatic and previews nothing.
				if (details.reason !== 'none') preview(option);
			}}
			onOpenChange={(next) => {
				if (!next && open) close();
			}}
			onValueChange={(option) => {
				if (option !== null) keep(option);
			}}
		>
			{/* Settings' landing places focus when the list closes, so Base UI must not return it. */}
			<ComboboxContent
				align='center'
				anchor={anchor}
				aria-label={intl.formatMessage(
					list.polarity === 'dark'
						? terminalSettingsMessages.pageThemeChooseDark
						: terminalSettingsMessages.pageThemeChooseLight,
				)}
				className={styles['popup']}
				finalFocus={false}
				side='bottom'
				sideOffset={TOP_OFFSET}
			>
				<ComboboxSearch
					aria-label={list.label}
					placeholder={intl.formatMessage(
						list.polarity === 'dark'
							? terminalSettingsMessages.pageFilterDarkThemes
							: terminalSettingsMessages.pageFilterLightThemes,
					)}
					spellCheck={false}
				/>
				{options.length === 0 ? <SettingsListFilterEmpty /> : null}
				<ComboboxList>
					{(option: TerminalColorThemeOptionView) => (
						<ComboboxItem key={option.id} value={option}>
							{option.name}
						</ComboboxItem>
					)}
				</ComboboxList>
			</ComboboxContent>
		</Combobox>
	);
}
