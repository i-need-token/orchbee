import {
	Button,
	Input,
	NativeSelect,
	NativeSelectOption,
	Slider,
	Switch,
} from '@orchbee/ui/components';
import * as React from 'react';

import { TERMINAL_FONT_WEIGHT_OPTIONS } from '../../../terminal-settings';
import type { TerminalSettingsSurfaceView } from '../../types/terminal-settings-surface';
import { SettingsSection } from '../settings-section/SettingsSection';
import { TerminalSettingsRow } from '../terminal-settings-row/TerminalSettingsRow';
import { ColorRow } from './components/color-row/ColorRow';
import { ColorThemePicker } from './components/color-theme-picker/ColorThemePicker';
import { FieldControl } from './components/field-control/FieldControl';
import styles from './TerminalSettingsSections.module.css';

interface TerminalSettingsSectionsProps {
	readonly view: TerminalSettingsSurfaceView;
}

export function TerminalSettingsSections({ view }: TerminalSettingsSectionsProps): React.ReactNode {
	return (
		<div className={styles['root']}>
			<SettingsSection
				title={view.messages.terminalHistorySection}
				visible={
					view.category === 'terminal' &&
					(view.scrollbackBytesVisible ||
						view.trimTrailingBlankRowsForAgentsVisible ||
						view.resumeAgentsOnRestoreVisible ||
						view.restoreTerminalOutputVisible)
				}
			>
				<TerminalSettingsRow
					description={view.messages.scrollbackBytesDescription}
					label={view.messages.scrollbackBytesLabel}
					settingId='terminal.scrollback'
					visible={view.scrollbackBytesVisible}
				>
					<div className={styles['stackedControl']}>
						<FieldControl
							error={view.scrollbackBytesInvalid ? view.messages.scrollbackBytesInvalid : null}
							errorId='terminal-scrollback-error'
						>
							<Input
								aria-describedby={
									view.scrollbackBytesInvalid
										? 'terminal-scrollback-device-notice terminal-scrollback-error'
										: 'terminal-scrollback-device-notice'
								}
								aria-label={view.messages.scrollbackBytesLabel}
								aria-invalid={view.scrollbackBytesInvalid}
								max={1_000_000_000}
								min={0}
								type='number'
								value={view.scrollbackBytesDraft}
								onBlur={view.onScrollbackBytesBlur}
								onChange={view.onScrollbackBytesChange}
								onKeyDown={view.onScrollbackBytesKeyDown}
							/>
						</FieldControl>
						<span className={styles['deviceNotice']} id='terminal-scrollback-device-notice'>
							{view.messages.scrollbackBytesFutureNotice}
						</span>
					</div>
				</TerminalSettingsRow>
				<TerminalSettingsRow
					description={view.messages.trimTrailingBlankRowsForAgentsDescription}
					label={view.messages.trimTrailingBlankRowsForAgentsLabel}
					settingId='terminal.trimTrailingBlankRowsForAgents'
					visible={view.trimTrailingBlankRowsForAgentsVisible}
				>
					<Switch
						aria-label={view.messages.trimTrailingBlankRowsForAgentsLabel}
						checked={view.trimTrailingBlankRowsForAgents}
						onCheckedChange={view.onTrimTrailingBlankRowsForAgentsChange}
					/>
				</TerminalSettingsRow>
				<TerminalSettingsRow
					description={view.messages.resumeAgentsOnRestoreDescription}
					label={view.messages.resumeAgentsOnRestoreLabel}
					settingId='terminal.resumeAgentsOnRestore'
					visible={view.resumeAgentsOnRestoreVisible}
				>
					<Switch
						aria-label={view.messages.resumeAgentsOnRestoreLabel}
						checked={view.resumeAgentsOnRestore}
						onCheckedChange={view.onResumeAgentsOnRestoreChange}
					/>
				</TerminalSettingsRow>
				<TerminalSettingsRow
					description={view.messages.restoreTerminalOutputDescription}
					label={view.messages.restoreTerminalOutputLabel}
					settingId='terminal.restoreTerminalOutput'
					visible={view.restoreTerminalOutputVisible}
				>
					<Switch
						aria-label={view.messages.restoreTerminalOutputLabel}
						checked={view.restoreTerminalOutput}
						onCheckedChange={view.onRestoreTerminalOutputChange}
					/>
				</TerminalSettingsRow>
			</SettingsSection>
			<SettingsSection
				title={view.messages.applicationAppearanceSection}
				visible={view.category === 'appearance' && view.applicationThemeVisible}
			>
				<TerminalSettingsRow
					description={view.messages.applicationThemeDescription}
					label={view.messages.applicationThemeLabel}
					settingId='appearance.application-theme'
					visible={view.applicationThemeVisible}
				>
					<NativeSelect
						aria-label={view.messages.applicationThemeLabel}
						className={styles['wideControl']}
						value={view.preferences.applicationTheme}
						onChange={view.onApplicationThemeChange}
					>
						<NativeSelectOption value='system'>
							{view.messages.applicationThemeSystem}
						</NativeSelectOption>
						<NativeSelectOption value='light'>
							{view.messages.applicationThemeLight}
						</NativeSelectOption>
						<NativeSelectOption value='dark'>
							{view.messages.applicationThemeDark}
						</NativeSelectOption>
					</NativeSelect>
				</TerminalSettingsRow>
				{view.colorThemePickers.map((picker) => {
					const noticeId = picker.notice === null ? undefined : `${picker.settingId}-notice`;
					return (
						<TerminalSettingsRow
							controlAlignment='start'
							description={picker.description}
							key={picker.settingId}
							label={picker.label}
							settingId={picker.settingId}
							visible={view.applicationThemeVisible}
						>
							<div className={styles['stackedControl']}>
								<ColorThemePicker describedBy={noticeId} picker={picker} />
								{picker.notice === null ? null : (
									<span className={styles['deviceNotice']} id={noticeId}>
										{picker.notice}
									</span>
								)}
							</div>
						</TerminalSettingsRow>
					);
				})}
			</SettingsSection>
			<SettingsSection
				title={view.messages.terminalLayoutSection}
				visible={view.category === 'appearance' && view.terminalPaddingVisible}
			>
				<TerminalSettingsRow
					description={view.messages.terminalPaddingDescription}
					label={view.messages.terminalPaddingLabel}
					settingId='appearance.terminalPadding'
					visible={view.terminalPaddingVisible}
				>
					<FieldControl
						error={view.terminalPaddingInvalid ? view.messages.terminalPaddingInvalid : null}
						errorId='terminal-padding-error'
					>
						<Input
							aria-describedby={view.terminalPaddingInvalid ? 'terminal-padding-error' : undefined}
							aria-label={view.messages.terminalPaddingLabel}
							aria-invalid={view.terminalPaddingInvalid}
							max={32}
							min={0}
							type='number'
							value={view.terminalPaddingDraft}
							onBlur={view.onTerminalPaddingBlur}
							onChange={view.onTerminalPaddingChange}
							onKeyDown={view.onTerminalPaddingKeyDown}
						/>
					</FieldControl>
				</TerminalSettingsRow>
			</SettingsSection>

			<SettingsSection
				title={view.messages.terminalTypographySection}
				visible={
					view.category === 'text' &&
					(view.fontFamilyVisible ||
						view.fontSizeVisible ||
						view.fontWeightVisible ||
						view.fontWeightBoldVisible)
				}
			>
				<TerminalSettingsRow
					description={`${view.messages.fontFamilyDescription} ${view.messages.fontFamilyDeviceAvailability}`}
					label={view.messages.fontFamilyLabel}
					settingId='text.font-family'
					visible={view.fontFamilyVisible}
				>
					<FieldControl
						error={view.fontFamilyInvalid ? view.messages.fontFamilyInvalid : null}
						errorId='terminal-font-family-error'
					>
						<Input
							aria-describedby={view.fontFamilyInvalid ? 'terminal-font-family-error' : undefined}
							aria-label={view.messages.fontFamilyLabel}
							aria-invalid={view.fontFamilyInvalid}
							value={view.fontFamilyDraft}
							onBlur={view.onFontFamilyBlur}
							onChange={view.onFontFamilyChange}
							onKeyDown={view.onFontFamilyKeyDown}
						/>
					</FieldControl>
				</TerminalSettingsRow>
				<TerminalSettingsRow
					description={view.messages.fontSizeDescription}
					label={view.messages.fontSizeLabel}
					settingId='text.font-size'
					visible={view.fontSizeVisible}
				>
					<FieldControl
						error={view.fontSizeInvalid ? view.messages.fontSizeRangeInvalid : null}
						errorId='terminal-font-size-error'
					>
						<Input
							aria-describedby={view.fontSizeInvalid ? 'terminal-font-size-error' : undefined}
							aria-label={view.messages.fontSizeLabel}
							aria-invalid={view.fontSizeInvalid}
							max={48}
							min={8}
							type='number'
							value={view.fontSizeDraft}
							onBlur={view.onFontSizeBlur}
							onChange={view.onFontSizeChange}
							onKeyDown={view.onFontSizeKeyDown}
						/>
					</FieldControl>
				</TerminalSettingsRow>
				<TerminalSettingsRow
					description={view.messages.fontWeightDescription}
					label={view.messages.fontWeightLabel}
					settingId='text.font-weight'
					visible={view.fontWeightVisible}
				>
					<NativeSelect
						aria-label={view.messages.fontWeightLabel}
						className={styles['wideControl']}
						value={String(view.preferences.fontWeight)}
						onChange={view.onFontWeightChange}
					>
						{TERMINAL_FONT_WEIGHT_OPTIONS.map((weight) => (
							<NativeSelectOption key={weight} value={String(weight)}>
								{weight}
							</NativeSelectOption>
						))}
					</NativeSelect>
				</TerminalSettingsRow>
				<TerminalSettingsRow
					description={view.messages.fontWeightBoldDescription}
					label={view.messages.fontWeightBoldLabel}
					settingId='text.font-weight-bold'
					visible={view.fontWeightBoldVisible}
				>
					<NativeSelect
						aria-label={view.messages.fontWeightBoldLabel}
						className={styles['wideControl']}
						value={String(view.preferences.fontWeightBold)}
						onChange={view.onFontWeightBoldChange}
					>
						{TERMINAL_FONT_WEIGHT_OPTIONS.map((weight) => (
							<NativeSelectOption key={weight} value={String(weight)}>
								{weight}
							</NativeSelectOption>
						))}
					</NativeSelect>
				</TerminalSettingsRow>
			</SettingsSection>
			<SettingsSection
				title={view.messages.terminalTextShapingSection}
				visible={view.category === 'text' && view.fontLigaturesVisible}
			>
				<TerminalSettingsRow
					description={view.messages.fontLigaturesDescription}
					label={view.messages.fontLigaturesLabel}
					settingId='text.ligatures'
					visible={view.fontLigaturesVisible}
				>
					<Switch
						aria-label={view.messages.fontLigaturesLabel}
						checked={view.preferences.fontLigatures}
						onCheckedChange={view.onFontLigaturesChange}
					/>
				</TerminalSettingsRow>
			</SettingsSection>

			<SettingsSection
				title={view.messages.terminalClipboardSection}
				visible={view.category === 'interaction' && view.copyOnSelectVisible}
			>
				<TerminalSettingsRow
					description={view.messages.copyOnSelectDescription}
					label={view.messages.copyOnSelectLabel}
					settingId='interaction.copyOnSelect'
					visible={view.copyOnSelectVisible}
				>
					<Switch
						aria-label={view.messages.copyOnSelectLabel}
						checked={view.copyOnSelect}
						onCheckedChange={view.onCopyOnSelectChange}
					/>
				</TerminalSettingsRow>
			</SettingsSection>

			<SettingsSection
				title={view.messages.terminalColorsSection}
				visible={
					view.category === 'colors' &&
					(view.displayThemeVisible ||
						view.foregroundVisible ||
						view.backgroundVisible ||
						view.opacityVisible)
				}
			>
				<TerminalSettingsRow
					description={view.messages.terminalDisplayThemeDescription}
					label={view.messages.terminalDisplayThemeLabel}
					settingId='colors.display-theme'
					visible={view.displayThemeVisible}
				>
					<div className={styles['stackedControl']}>
						<NativeSelect
							aria-label={view.messages.terminalDisplayThemeLabel}
							className={styles['wideControl']}
							value={view.preferences.terminalDisplayTheme}
							onChange={view.onTerminalDisplayThemeChange}
						>
							<NativeSelectOption value='theme'>
								{view.messages.terminalDisplayThemeTheme}
							</NativeSelectOption>
							<NativeSelectOption value='host'>
								{view.messages.terminalDisplayThemeHost}
							</NativeSelectOption>
							<NativeSelectOption value='custom'>
								{view.messages.terminalDisplayThemeCustom}
							</NativeSelectOption>
						</NativeSelect>
						{view.preferences.terminalDisplayTheme === 'custom' ? (
							<span className={styles['deviceNotice']}>
								{view.messages.currentDeviceOnlyNotice}
							</span>
						) : null}
					</div>
				</TerminalSettingsRow>
				<ColorRow
					color={view.preferences.customForeground}
					description={view.messages.foregroundDescription}
					disabled={!view.customColorsEnabled}
					draft={view.foregroundDraft}
					error={view.foregroundInvalid ? view.messages.foregroundInvalid : null}
					label={view.messages.foregroundLabel}
					settingId='colors.foreground'
					visible={view.foregroundVisible}
					onColorChange={view.onForegroundColorChange}
					onDraftChange={view.onForegroundDraftChange}
					onDraftKeyDown={view.onForegroundDraftKeyDown}
					onInputBlur={view.onForegroundInputBlur}
				/>
				<ColorRow
					color={view.preferences.customBackground}
					description={view.messages.backgroundDescription}
					disabled={!view.customColorsEnabled}
					draft={view.backgroundDraft}
					error={view.backgroundInvalid ? view.messages.backgroundInvalid : null}
					label={view.messages.backgroundLabel}
					settingId='colors.background'
					visible={view.backgroundVisible}
					onColorChange={view.onBackgroundColorChange}
					onDraftChange={view.onBackgroundDraftChange}
					onDraftKeyDown={view.onBackgroundDraftKeyDown}
					onInputBlur={view.onBackgroundInputBlur}
				/>
				<TerminalSettingsRow
					description={view.messages.backgroundOpacityDescription}
					label={view.messages.backgroundOpacityLabel}
					settingId='colors.background-opacity'
					visible={view.opacityVisible}
				>
					<div className={styles['rangeControl']}>
						<Slider
							aria-label={view.messages.backgroundOpacityLabel}
							className={styles['range']}
							max={100}
							min={10}
							value={[view.preferences.backgroundOpacityPercent]}
							onValueChange={view.onBackgroundOpacityChange}
						/>
						<output className={styles['rangeValue']}>
							{view.messages.backgroundOpacityPercentValue(
								view.preferences.backgroundOpacityPercent,
							)}
						</output>
					</div>
				</TerminalSettingsRow>
			</SettingsSection>

			<SettingsSection
				title={view.messages.reset}
				visible={view.category === 'general' && view.resetVisible}
			>
				<TerminalSettingsRow
					description={view.messages.resetDescription}
					label={view.messages.reset}
					settingId='general.reset'
					visible={view.resetVisible}
				>
					<Button variant='outline' onClick={view.onReset}>
						{view.messages.reset}
					</Button>
				</TerminalSettingsRow>
			</SettingsSection>
		</div>
	);
}
