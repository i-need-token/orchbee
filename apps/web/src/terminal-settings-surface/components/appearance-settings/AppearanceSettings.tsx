import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Input } from '@orchbee/ui/components';
import * as React from 'react';

import { windowTerminalColors, type TerminalSettingsController } from '../../../terminal-settings';
import { TERMINAL_FONT_SIZE_RANGE } from '../../constants/terminal-font-size-range';
import { useSetting } from '../../hooks/useSetting';
import { useSystemPrefersDark } from '../../hooks/useSystemPrefersDark';
import type { TerminalSettingsFontCatalog } from '../../types/terminal-settings-surface';
import { FontFamilyPicker } from '../font-family-picker/FontFamilyPicker';
import { FontSizeStepper } from '../font-size-stepper/FontSizeStepper';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';
import { SettingsSelect } from '../settings-select/SettingsSelect';
import { TerminalPreview } from '../terminal-preview/TerminalPreview';
import { useAppearanceSettings } from './hooks/useAppearanceSettings';
import styles from './AppearanceSettings.module.css';

interface AppearanceSettingsProps {
	readonly controller: TerminalSettingsController;
	readonly fontCatalog: TerminalSettingsFontCatalog | undefined;
	readonly query: string;
}

/** The app appearance, terminal colors, and terminal text. */
export function AppearanceSettings({
	controller,
	fontCatalog,
	query,
}: AppearanceSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const view = useAppearanceSettings(controller);
	const applicationTheme = useSetting(controller.userSettings, 'appearance.mode');
	const displayTheme = useSetting(controller.userSettings, 'appearance.terminal_colors');
	const { preferences } = view;
	const applicationThemeLabel = intl.formatMessage(
		terminalSettingsMessages.pageApplicationThemeLabel,
	);
	const terminalColorsLabel = intl.formatMessage(terminalSettingsMessages.pageTerminalColorsLabel);
	const fontLabel = intl.formatMessage(terminalSettingsMessages.pageFontLabel);
	const fontSizeLabel = intl.formatMessage(terminalSettingsMessages.pageFontSizeLabel);
	const custom = preferences.terminalDisplayTheme === 'custom';
	const systemDark = useSystemPrefersDark();
	// The highlighted entry of the open font list, shown before it is chosen.
	const [fontPreview, setFontPreview] = React.useState<string | null>(null);
	const draftSize = Number(view.fontSize.draft);
	const previewSize =
		Number.isInteger(draftSize) &&
		draftSize >= TERMINAL_FONT_SIZE_RANGE.minimum &&
		draftSize <= TERMINAL_FONT_SIZE_RANGE.maximum
			? draftSize
			: preferences.fontSize;
	const preview =
		query.trim() === '' ? (
			<TerminalPreview
				colors={windowTerminalColors(view.presented, systemDark, controller.colorThemes)}
				fontFamily={fontPreview ?? preferences.fontFamily}
				fontSize={previewSize}
				fontWeight={preferences.fontWeight}
				label={intl.formatMessage(terminalSettingsMessages.pagePreviewLabel)}
			/>
		) : undefined;
	return (
		<>
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupAppAppearance)}>
				<SettingsRow
					description={intl.formatMessage(terminalSettingsMessages.pageApplicationThemeDescription)}
					label={applicationThemeLabel}
					modified={applicationTheme.modified}
					query={query}
					settingId='appearance.application-theme'
					status={<SettingFailure message={applicationTheme.failureMessage} />}
					onRestore={applicationTheme.readOnly ? undefined : applicationTheme.restore}
				>
					<SettingsSelect
						disabled={applicationTheme.readOnly}
						label={applicationThemeLabel}
						options={[
							{ label: intl.formatMessage(terminalSettingsMessages.sharedSystem), value: 'system' },
							{
								label: intl.formatMessage(terminalSettingsMessages.applicationThemeLight),
								value: 'light',
							},
							{
								label: intl.formatMessage(terminalSettingsMessages.applicationThemeDark),
								value: 'dark',
							},
						]}
						value={preferences.applicationTheme}
						onValueChange={(value): void => {
							controller.update({ applicationTheme: value });
						}}
					/>
				</SettingsRow>
			</SettingsGroup>
			<SettingsGroup label={intl.formatMessage(terminalSettingsMessages.pageGroupTerminalColors)}>
				<SettingsRow
					description={intl.formatMessage(terminalSettingsMessages.pageTerminalColorsDescription)}
					label={terminalColorsLabel}
					modified={displayTheme.modified}
					query={query}
					settingId='appearance.terminal-colors'
					status={<SettingFailure message={displayTheme.failureMessage} />}
					onRestore={displayTheme.readOnly ? undefined : displayTheme.restore}
				>
					<SettingsSelect
						disabled={displayTheme.readOnly}
						label={terminalColorsLabel}
						options={[
							{
								label: intl.formatMessage(terminalSettingsMessages.pageTerminalColorsTheme),
								value: 'theme',
							},
							{
								label: intl.formatMessage(terminalSettingsMessages.pageTerminalColorsHost),
								value: 'host',
							},
							{
								label: intl.formatMessage(terminalSettingsMessages.pageTerminalColorsCustom),
								value: 'custom',
							},
						]}
						value={preferences.terminalDisplayTheme}
						onValueChange={(value): void => {
							controller.update({ terminalDisplayTheme: value });
						}}
					/>
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
				{/* Size comes first, as in the reference, so the growing preview never moves the stepper. */}
				<SettingsRow
					description={intl.formatMessage(terminalSettingsMessages.pageFontSizeDescription)}
					label={fontSizeLabel}
					modified={view.fontSize.modified}
					query={query}
					settingId='appearance.font-size'
					status={<SettingFailure message={view.fontSize.error} />}
					onRestore={view.fontSize.onRestore}
				>
					<FontSizeStepper
						draft={view.fontSize.draft}
						invalid={view.fontSize.error !== null}
						label={fontSizeLabel}
						maximum={TERMINAL_FONT_SIZE_RANGE.maximum}
						minimum={TERMINAL_FONT_SIZE_RANGE.minimum}
						readOnly={view.fontSize.readOnly}
						value={view.fontSize.value}
						onBlur={view.fontSize.onBlur}
						onChange={view.fontSize.onChange}
						onKeyDown={view.fontSize.onKeyDown}
						onStep={view.fontSize.onStep}
					/>
				</SettingsRow>
				<SettingsRow
					description={intl.formatMessage(terminalSettingsMessages.pageFontDescription)}
					label={fontLabel}
					modified={view.fontFamily.modified}
					preview={preview}
					query={query}
					settingId='appearance.font'
					status={<SettingFailure message={view.fontFamily.error} />}
					onRestore={view.fontFamily.onRestore}
				>
					<FontFamilyPicker
						catalog={fontCatalog}
						invalid={view.fontFamily.error !== null}
						label={fontLabel}
						modified={view.fontFamily.modified}
						readOnly={view.fontFamily.readOnly}
						value={view.fontFamily.value}
						onChoose={view.fontFamily.onChoose}
						onPreview={setFontPreview}
						onRestore={view.fontFamily.onChooseBuiltIn}
					/>
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
			status={<SettingFailure message={draft.error} />}
			onRestore={draft.onRestore}
		>
			<span className={styles['colorField']}>
				<input
					aria-label={label}
					className={styles['colorPicker']}
					disabled={draft.readOnly}
					type='color'
					value={draft.color}
					onChange={draft.onPick}
				/>
				<Input
					aria-invalid={draft.error !== null}
					aria-label={label}
					className={styles['hexInput']}
					readOnly={draft.readOnly}
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
