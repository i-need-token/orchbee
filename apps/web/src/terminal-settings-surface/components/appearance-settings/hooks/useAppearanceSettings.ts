import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import {
	DEFAULT_TERMINAL_APPEARANCE_PREFERENCES,
	normalizeTerminalHexColor,
	useTerminalSettings,
	type TerminalAppearancePreferences,
	type TerminalSettingsController,
} from '../../../../terminal-settings';
import { TERMINAL_FONT_SIZE_RANGE } from '../../../constants/terminal-font-size-range';
import { useSharedPreferenceDraft } from '../../../hooks/useSharedPreferenceDraft';

/** The terminal font sizes the appearance model admits. */

interface TextDraftView {
	readonly draft: string;
	readonly error: string | null;
	readonly modified: boolean;
	readonly onBlur: () => void;
	readonly onChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onRestore: () => void;
}

interface ColorDraftView extends TextDraftView {
	/** The confirmed color, for the native color picker. */
	readonly color: string;
	readonly onPick: React.ChangeEventHandler<HTMLInputElement>;
}

interface AppearanceSettingsView {
	readonly background: ColorDraftView;
	readonly fontFamily: TextDraftView;
	readonly fontSize: TextDraftView;
	readonly foreground: ColorDraftView;
	readonly preferences: TerminalAppearancePreferences;
}

/**
 * Drafts for the typed appearance values. A draft commits on Enter or when its field loses focus;
 * an invalid draft stays visible with its message and changes nothing.
 */
export function useAppearanceSettings(
	controller: TerminalSettingsController,
): AppearanceSettingsView {
	const intl = useOrchbeeIntl();
	const { preferences } = useTerminalSettings(controller);
	const [fontFamilyDraft, setFontFamilyDraft] = React.useState(preferences.fontFamily);
	const [fontFamilyInvalid, setFontFamilyInvalid] = React.useState(false);
	const [fontSizeDraft, setFontSizeDraft] = React.useState(String(preferences.fontSize));
	const [fontSizeInvalid, setFontSizeInvalid] = React.useState(false);
	const foreground = useSharedPreferenceDraft(
		preferences.customForeground,
		'customForeground',
		controller.userPreferences,
	);
	const [foregroundInvalid, setForegroundInvalid] = React.useState(false);
	const background = useSharedPreferenceDraft(
		preferences.customBackground,
		'customBackground',
		controller.userPreferences,
	);
	const [backgroundInvalid, setBackgroundInvalid] = React.useState(false);

	React.useEffect(() => {
		setFontFamilyDraft(preferences.fontFamily);
		setFontFamilyInvalid(false);
		setFontSizeDraft(String(preferences.fontSize));
		setFontSizeInvalid(false);
		setForegroundInvalid(false);
		setBackgroundInvalid(false);
	}, [preferences]);

	const commitFontFamily = (): void => {
		const result = controller.update({ fontFamily: fontFamilyDraft });
		setFontFamilyInvalid(result.kind === 'invalid');
	};
	const commitFontSize = (): void => {
		const result = controller.update({ fontSize: Number(fontSizeDraft) });
		setFontSizeInvalid(result.kind === 'invalid');
	};
	const colorDraft = (
		draft: ReturnType<typeof useSharedPreferenceDraft>,
		key: 'customBackground' | 'customForeground',
		invalid: boolean,
		setInvalid: (invalid: boolean) => void,
		invalidMessage: string,
	): ColorDraftView => {
		const submit = (value: string): void => {
			const normalized = normalizeTerminalHexColor(value);
			setInvalid(normalized === null);
			if (normalized !== null) draft.submit({ [key]: normalized });
		};
		return {
			color: preferences[key],
			draft: draft.draft,
			error: invalid ? invalidMessage : null,
			modified: preferences[key] !== DEFAULT_TERMINAL_APPEARANCE_PREFERENCES[key],
			onBlur: () => submit(draft.draft),
			onChange: (event) => draft.change(event.currentTarget.value),
			onKeyDown: commitOnEnter(() => submit(draft.draft)),
			onPick: (event) => {
				draft.change(event.currentTarget.value);
				submit(event.currentTarget.value);
			},
			onRestore: () => {
				void controller.userPreferences.reset([key]);
			},
		};
	};

	return {
		background: colorDraft(
			background,
			'customBackground',
			backgroundInvalid,
			setBackgroundInvalid,
			intl.formatMessage(terminalSettingsMessages.backgroundInvalid),
		),
		fontFamily: {
			draft: fontFamilyDraft,
			error: fontFamilyInvalid
				? intl.formatMessage(terminalSettingsMessages.fontFamilyInvalid)
				: null,
			modified: preferences.fontFamily !== DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontFamily,
			onBlur: commitFontFamily,
			onChange: (event) => setFontFamilyDraft(event.currentTarget.value),
			onKeyDown: commitOnEnter(commitFontFamily),
			onRestore: () => {
				controller.update({ fontFamily: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontFamily });
			},
		},
		fontSize: {
			draft: fontSizeDraft,
			error: fontSizeInvalid
				? intl.formatMessage(
						terminalSettingsMessages.fontSizeRangeInvalid,
						TERMINAL_FONT_SIZE_RANGE,
					)
				: null,
			modified: preferences.fontSize !== DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontSize,
			onBlur: commitFontSize,
			onChange: (event) => setFontSizeDraft(event.currentTarget.value),
			onKeyDown: commitOnEnter(commitFontSize),
			onRestore: () => {
				controller.update({ fontSize: DEFAULT_TERMINAL_APPEARANCE_PREFERENCES.fontSize });
			},
		},
		foreground: colorDraft(
			foreground,
			'customForeground',
			foregroundInvalid,
			setForegroundInvalid,
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
