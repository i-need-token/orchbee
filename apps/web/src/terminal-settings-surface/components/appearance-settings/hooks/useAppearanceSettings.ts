import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { SettingKey } from '@orchbee/workbench-client-protocol';
import * as React from 'react';

import {
	normalizeTerminalHexColor,
	useTerminalSettings,
	type TerminalAppearancePreferences,
	type TerminalSettingsController,
} from '../../../../terminal-settings';
import { TERMINAL_FONT_SIZE_RANGE } from '../../../constants/terminal-font-size-range';
import { useSetting, type SettingView } from '../../../hooks/useSetting';
import { useSettingDraft } from '../../../hooks/useSettingDraft';

interface TextDraftView {
	readonly draft: string;
	/** A draft that was not admitted, or why the file did not take the last change. */
	readonly error: string | null;
	readonly modified: boolean;
	readonly onBlur: () => void;
	readonly onChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	/** Undefined while the file is read-only, which hides the row's Restore default. */
	readonly onRestore: (() => void) | undefined;
	readonly readOnly: boolean;
}

interface ColorDraftView extends TextDraftView {
	/** The confirmed color, for the native color picker. */
	readonly color: string;
	readonly onPick: React.ChangeEventHandler<HTMLInputElement>;
}

/** What a draft needs of its setting; the draft commits through the controller, not set. */
type DraftSetting = Omit<SettingView<SettingKey>, 'set' | 'value'>;

interface AppearanceSettingsView {
	readonly background: ColorDraftView;
	readonly fontFamily: TextDraftView;
	readonly fontSize: TextDraftView;
	readonly foreground: ColorDraftView;
	readonly preferences: TerminalAppearancePreferences;
}

/**
 * Drafts for the typed appearance values. A draft commits on Enter or when its field loses focus;
 * an invalid draft stays visible with its message and changes nothing, and a change the file refuses
 * puts the field back on the file's value with the reason.
 */
export function useAppearanceSettings(
	controller: TerminalSettingsController,
): AppearanceSettingsView {
	const intl = useOrchbeeIntl();
	const { preferences } = useTerminalSettings(controller);
	const settings = controller.userSettings;
	const fontFamily = useSetting(settings, 'terminal.font_family');
	const fontSize = useSetting(settings, 'terminal.font_size');
	const foreground = useSetting(settings, 'appearance.custom_foreground');
	const background = useSetting(settings, 'appearance.custom_background');
	const fontFamilyDraft = useSettingDraft(preferences.fontFamily, fontFamily.failure);
	const fontSizeDraft = useSettingDraft(String(preferences.fontSize), fontSize.failure);
	const foregroundDraft = useSettingDraft(preferences.customForeground, foreground.failure);
	const backgroundDraft = useSettingDraft(preferences.customBackground, background.failure);
	const [invalid, setInvalid] = React.useState<ReadonlySet<string>>(new Set());

	React.useEffect(() => {
		setInvalid(new Set());
	}, [preferences]);

	const markInvalid = (key: string, value: boolean): void => {
		setInvalid((current) => {
			if (current.has(key) === value) return current;
			const next = new Set(current);
			if (value) next.add(key);
			else next.delete(key);
			return next;
		});
	};
	const textDraft = (
		setting: DraftSetting,
		key: string,
		draft: ReturnType<typeof useSettingDraft>,
		invalidMessage: string,
		commit: (draft: string) => void,
	): TextDraftView => ({
		draft: draft.draft,
		error: invalid.has(key) ? invalidMessage : setting.failureMessage,
		modified: setting.modified,
		onBlur: () => commit(draft.draft),
		onChange: (event) => draft.change(event.currentTarget.value),
		onKeyDown: commitOnEnter(() => commit(draft.draft)),
		onRestore: setting.readOnly ? undefined : setting.restore,
		readOnly: setting.readOnly,
	});
	const colorDraft = (
		setting: DraftSetting,
		key: 'customBackground' | 'customForeground',
		draft: ReturnType<typeof useSettingDraft>,
		invalidMessage: string,
	): ColorDraftView => {
		const submit = (value: string): void => {
			const normalized = normalizeTerminalHexColor(value);
			markInvalid(key, normalized === null);
			if (normalized !== null) controller.update({ [key]: normalized });
		};
		return {
			...textDraft(setting, key, draft, invalidMessage, submit),
			color: preferences[key],
			onPick: (event) => {
				draft.change(event.currentTarget.value);
				submit(event.currentTarget.value);
			},
		};
	};

	return {
		background: colorDraft(
			background,
			'customBackground',
			backgroundDraft,
			intl.formatMessage(terminalSettingsMessages.backgroundInvalid),
		),
		fontFamily: textDraft(
			fontFamily,
			'terminal.font_family',
			fontFamilyDraft,
			intl.formatMessage(terminalSettingsMessages.fontFamilyInvalid),
			(draft) => {
				markInvalid(
					'terminal.font_family',
					controller.update({ fontFamily: draft }).kind === 'invalid',
				);
			},
		),
		fontSize: textDraft(
			fontSize,
			'terminal.font_size',
			fontSizeDraft,
			intl.formatMessage(terminalSettingsMessages.fontSizeRangeInvalid, TERMINAL_FONT_SIZE_RANGE),
			(draft) => {
				markInvalid(
					'terminal.font_size',
					controller.update({ fontSize: Number(draft) }).kind === 'invalid',
				);
			},
		),
		foreground: colorDraft(
			foreground,
			'customForeground',
			foregroundDraft,
			intl.formatMessage(terminalSettingsMessages.foregroundInvalid),
		),
		preferences,
	};
}

function commitOnEnter(commit: () => void): React.KeyboardEventHandler<HTMLInputElement> {
	return (event) => {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		commit();
		event.currentTarget.blur();
	};
}
