import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Input, NativeSelect, NativeSelectOption } from '@orchbee/ui/components';
import type * as React from 'react';

import type { TerminalSettingsController } from '../../../terminal-settings';
import { TERMINAL_FONT_SIZE_RANGE } from '../../constants/terminal-font-size-range';
import { useSharedPreferenceDefault } from '../../hooks/useSharedPreferenceDefault';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';
import { useAppearanceSettings } from './hooks/useAppearanceSettings';
import styles from './AppearanceSettings.module.css';

interface AppearanceSettingsProps {
	readonly controller: TerminalSettingsController;
	readonly query: string;
}

/** The app appearance, terminal colors, and terminal text. */
export function AppearanceSettings({
	controller,
	query,
}: AppearanceSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const view = useAppearanceSettings(controller);
	const applicationTheme = useSharedPreferenceDefault(
		controller.userPreferences,
		'applicationTheme',
	);
	const displayTheme = useSharedPreferenceDefault(
		controller.userPreferences,
		'terminalDisplayTheme',
	);
	const { preferences } = view;
	const applicationThemeLabel = intl.formatMessage(
		terminalSettingsMessages.pageApplicationThemeLabel,
	);
	const terminalColorsLabel = intl.formatMessage(terminalSettingsMessages.pageTerminalColorsLabel);
	const fontLabel = intl.formatMessage(terminalSettingsMessages.pageFontLabel);
	const fontSizeLabel = intl.formatMessage(terminalSettingsMessages.pageFontSizeLabel);
	const custom = preferences.terminalDisplayTheme === 'custom';
	return (
		<>
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupAppAppearance)}>
				<SettingsRow
					description={intl.formatMessage(terminalSettingsMessages.pageApplicationThemeDescription)}
					label={applicationThemeLabel}
					modified={applicationTheme.modified}
					query={query}
					settingId='appearance.application-theme'
					onRestore={applicationTheme.restore}
				>
					<NativeSelect
						aria-label={applicationThemeLabel}
						size='sm'
						value={preferences.applicationTheme}
						onChange={(event): void => {
							const value = event.currentTarget.value;
							if (value === 'system' || value === 'light' || value === 'dark') {
								controller.update({ applicationTheme: value });
							}
						}}
					>
						<NativeSelectOption value='system'>
							{intl.formatMessage(terminalSettingsMessages.sharedSystem)}
						</NativeSelectOption>
						<NativeSelectOption value='light'>
							{intl.formatMessage(terminalSettingsMessages.applicationThemeLight)}
						</NativeSelectOption>
						<NativeSelectOption value='dark'>
							{intl.formatMessage(terminalSettingsMessages.applicationThemeDark)}
						</NativeSelectOption>
					</NativeSelect>
				</SettingsRow>
			</SettingsGroup>
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupTerminalColors)}>
				<SettingsRow
					description={intl.formatMessage(terminalSettingsMessages.pageTerminalColorsDescription)}
					label={terminalColorsLabel}
					modified={displayTheme.modified}
					query={query}
					settingId='appearance.terminal-colors'
					onRestore={displayTheme.restore}
				>
					<NativeSelect
						aria-label={terminalColorsLabel}
						size='sm'
						value={preferences.terminalDisplayTheme}
						onChange={(event): void => {
							const value = event.currentTarget.value;
							if (value === 'theme' || value === 'host' || value === 'custom') {
								controller.update({ terminalDisplayTheme: value });
							}
						}}
					>
						<NativeSelectOption value='theme'>
							{intl.formatMessage(terminalSettingsMessages.pageTerminalColorsTheme)}
						</NativeSelectOption>
						<NativeSelectOption value='host'>
							{intl.formatMessage(terminalSettingsMessages.pageTerminalColorsHost)}
						</NativeSelectOption>
						<NativeSelectOption value='custom'>
							{intl.formatMessage(terminalSettingsMessages.pageTerminalColorsCustom)}
						</NativeSelectOption>
					</NativeSelect>
				</SettingsRow>
				{custom ? (
					<ColorRow
						description={intl.formatMessage(terminalSettingsMessages.pageTextColorDescription)}
						draft={view.foreground}
						label={intl.formatMessage(terminalSettingsMessages.pageTextColorLabel)}
						query={query}
						settingId='appearance.text-color'
					/>
				) : null}
				{custom ? (
					<ColorRow
						description={intl.formatMessage(
							terminalSettingsMessages.pageBackgroundColorDescription,
						)}
						draft={view.background}
						label={intl.formatMessage(terminalSettingsMessages.pageBackgroundColorLabel)}
						query={query}
						settingId='appearance.background-color'
					/>
				) : null}
			</SettingsGroup>
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupTerminalText)}>
				<SettingsRow
					description={intl.formatMessage(terminalSettingsMessages.pageFontDescription)}
					label={fontLabel}
					modified={view.fontFamily.modified}
					query={query}
					settingId='appearance.font'
					status={<FieldError message={view.fontFamily.error} />}
					onRestore={view.fontFamily.onRestore}
				>
					<Input
						aria-invalid={view.fontFamily.error !== null}
						aria-label={fontLabel}
						className={styles['fontInput']}
						spellCheck={false}
						value={view.fontFamily.draft}
						onBlur={view.fontFamily.onBlur}
						onChange={view.fontFamily.onChange}
						onKeyDown={view.fontFamily.onKeyDown}
					/>
				</SettingsRow>
				<SettingsRow
					description={intl.formatMessage(terminalSettingsMessages.pageFontSizeDescription)}
					label={fontSizeLabel}
					modified={view.fontSize.modified}
					query={query}
					settingId='appearance.font-size'
					status={<FieldError message={view.fontSize.error} />}
					onRestore={view.fontSize.onRestore}
				>
					<span className={styles['unitField']}>
						<Input
							aria-invalid={view.fontSize.error !== null}
							aria-label={fontSizeLabel}
							className={styles['numberInput']}
							max={TERMINAL_FONT_SIZE_RANGE.maximum}
							min={TERMINAL_FONT_SIZE_RANGE.minimum}
							type='number'
							value={view.fontSize.draft}
							onBlur={view.fontSize.onBlur}
							onChange={view.fontSize.onChange}
							onKeyDown={view.fontSize.onKeyDown}
						/>
						<span className={styles['unit']}>
							{intl.formatMessage(terminalSettingsMessages.pageUnitPixels)}
						</span>
					</span>
				</SettingsRow>
			</SettingsGroup>
		</>
	);
}

interface ColorRowProps {
	readonly description: string;
	readonly draft: ReturnType<typeof useAppearanceSettings>['foreground'];
	readonly label: string;
	readonly query: string;
	readonly settingId: string;
}

/** A custom terminal color: the system color picker beside its hex value. */
function ColorRow({ description, draft, label, query, settingId }: ColorRowProps): React.ReactNode {
	return (
		<SettingsRow
			description={description}
			label={label}
			modified={draft.modified}
			query={query}
			settingId={settingId}
			status={<FieldError message={draft.error} />}
			onRestore={draft.onRestore}
		>
			<span className={styles['colorField']}>
				<input
					aria-label={label}
					className={styles['colorPicker']}
					type='color'
					value={draft.color}
					onChange={draft.onPick}
				/>
				<Input
					aria-invalid={draft.error !== null}
					aria-label={label}
					className={styles['hexInput']}
					spellCheck={false}
					value={draft.draft}
					onBlur={draft.onBlur}
					onChange={draft.onChange}
					onKeyDown={draft.onKeyDown}
				/>
			</span>
		</SettingsRow>
	);
}

function FieldError({ message }: { readonly message: string | null }): React.ReactNode {
	if (message === null) return null;
	return (
		<div className={styles['error']} role='alert'>
			{message}
		</div>
	);
}
