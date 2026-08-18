import { Button, Input, NativeSelect, NativeSelectOption, Slider } from '@orchbee/ui/components';
import * as React from 'react';

import type { TerminalSettingsSurfaceView } from '../../types/terminal-settings-surface';
import { SettingsSection } from '../settings-section/SettingsSection';
import { TerminalSettingsRow } from '../terminal-settings-row/TerminalSettingsRow';
import { ColorRow } from './components/color-row/ColorRow';
import { FieldControl } from './components/field-control/FieldControl';
import styles from './TerminalSettingsSections.module.css';

interface TerminalSettingsSectionsProps {
	readonly view: TerminalSettingsSurfaceView;
}

export function TerminalSettingsSections({ view }: TerminalSettingsSectionsProps): React.ReactNode {
	return (
		<div className={styles['root']}>
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
						<NativeSelectOption value='light'>
							{view.messages.applicationThemeLight}
						</NativeSelectOption>
						<NativeSelectOption value='dark'>
							{view.messages.applicationThemeDark}
						</NativeSelectOption>
					</NativeSelect>
				</TerminalSettingsRow>
			</SettingsSection>

			<SettingsSection
				title={view.messages.terminalTypographySection}
				visible={view.category === 'text' && (view.fontFamilyVisible || view.fontSizeVisible)}
			>
				<TerminalSettingsRow
					description={`${view.messages.fontFamilyDescription} ${view.messages.fontFamilyDeviceAvailability}`}
					label={view.messages.fontFamilyLabel}
					settingId='text.font-family'
					visible={view.fontFamilyVisible}
				>
					<FieldControl error={view.fontFamilyInvalid ? view.messages.fontFamilyInvalid : null}>
						<Input
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
					<FieldControl error={view.fontSizeInvalid ? view.messages.fontSizeRangeInvalid : null}>
						<Input
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
							<NativeSelectOption value='host'>
								{view.messages.terminalDisplayThemeHost}
							</NativeSelectOption>
							<NativeSelectOption value='light'>
								{view.messages.terminalDisplayThemeLight}
							</NativeSelectOption>
							<NativeSelectOption value='dark'>
								{view.messages.terminalDisplayThemeDark}
							</NativeSelectOption>
							<NativeSelectOption value='custom'>
								{view.messages.terminalDisplayThemeCustom}
							</NativeSelectOption>
						</NativeSelect>
						{view.preferences.terminalDisplayTheme === 'host' ? null : (
							<span className={styles['deviceNotice']}>
								{view.messages.currentDeviceOnlyNotice}
							</span>
						)}
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
