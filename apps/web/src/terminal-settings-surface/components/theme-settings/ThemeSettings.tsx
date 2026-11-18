import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import * as React from 'react';

import {
	useTerminalSettings,
	windowTerminalColors,
	type TerminalSettingsController,
} from '../../../terminal-settings';
import { useSetting } from '../../hooks/useSetting';
import { useSystemPrefersDark } from '../../hooks/useSystemPrefersDark';
import type { TerminalColorThemeListView } from '../../types/terminal-settings-surface';
import { buildColorThemePickerViews } from '../../utils/buildColorThemePickerViews';
import { matchesSettingsQuery } from '../../utils/matchesSettingsQuery';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { TerminalPreview } from '../terminal-preview/TerminalPreview';
import styles from './ThemeSettings.module.css';

interface ThemeSettingsProps {
	readonly controller: TerminalSettingsController;
	readonly query: string;
}

/**
 * A terminal preview of the theme shown now, then one group per polarity. Each group names the
 * theme its slot holds and opens that polarity's list as a quick pick over the window; the group
 * the app appearance shows now is tagged in use.
 */
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
	});
	const { presented } = snapshot;
	const groups = lists.flatMap((list) => {
		if (
			query.trim() !== '' &&
			!matchesSettingsQuery(
				query,
				[list.label, ...list.options.map((option) => option.name)],
				intl.locale,
			)
		) {
			return [];
		}
		const slot = list.polarity === 'dark' ? dark : light;
		return [
			<SettingsGroup
				description={slot.failureMessage ?? list.note}
				key={list.settingId}
				label={list.label}
				modified={slot.modified}
				tag={list.inUse ? intl.formatMessage(terminalSettingsMessages.pageThemesInUse) : null}
				onRestore={readOnly ? undefined : slot.restore}
			>
				<ColorThemeChooser
					disabled={readOnly}
					list={list}
					onOpen={() => controller.openColorThemePicker(list.polarity)}
				/>
			</SettingsGroup>,
		];
	});
	return (
		<>
			{query.trim() === '' ? (
				<div className={styles['preview']}>
					<TerminalPreview
						colors={windowTerminalColors(presented, systemDark, controller.colorThemes)}
						fontFamily={presented.fontFamily}
						fontSize={presented.fontSize}
						fontWeight={presented.fontWeight}
						label={intl.formatMessage(terminalSettingsMessages.pagePreviewLabel)}
					/>
				</div>
			) : null}
			{groups}
		</>
	);
}

interface ColorThemeChooserProps {
	readonly disabled: boolean;
	readonly list: TerminalColorThemeListView;
	readonly onOpen: () => void;
}

/** The theme a group's slot holds, beside the button that opens the group's list. */
function ColorThemeChooser({ disabled, list, onOpen }: ColorThemeChooserProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const nameId = React.useId();
	const selected = list.options.find((option) => option.id === list.selectedId);
	return (
		<div className={styles['chooser']} data-settings-row>
			<span className={styles['name']} id={nameId}>
				{selected?.name ?? list.selectedId}
			</span>
			<Button
				aria-describedby={nameId}
				aria-haspopup='dialog'
				data-color-theme-trigger={list.polarity}
				disabled={disabled}
				size='sm'
				variant='outline'
				onClick={onOpen}
			>
				{intl.formatMessage(terminalSettingsMessages.pageThemeChoose)}
			</Button>
		</div>
	);
}
