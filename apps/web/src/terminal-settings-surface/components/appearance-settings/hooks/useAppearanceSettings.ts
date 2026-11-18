import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { SettingKey } from '@orchbee/workbench-client-protocol';
import * as React from 'react';

import {
	BUILT_IN_TERMINAL_FONT_LIST,
	normalizeTerminalHexColor,
	terminalFontListFromText,
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

interface FontFamilyView extends TextDraftView {
	/** The list the draft stores while it is admitted, for the preview; null while it is not. */
	readonly draftPreview: string | null;
	/** Picking an entry stores its list; null when that list is not admitted, which shows the notice. */
	readonly onChoose: (fontList: string | null) => void;
	/** Choosing the built-in font restores the default. */
	readonly onChooseBuiltIn: () => void;
}

interface FontSizeView extends TextDraftView {
	readonly onStep: (size: number) => void;
	/** The stored size, which the stepper's buttons move from. */
	readonly value: number;
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
	readonly fontFamily: FontFamilyView;
	readonly fontSize: FontSizeView;
	readonly foreground: ColorDraftView;
	readonly preferences: TerminalAppearancePreferences;
	/** The preferences the window shows now, a previewed theme included. */
	readonly presented: TerminalAppearancePreferences;
}

/**
 * Drafts for the typed appearance values and the font family's choice. A draft commits on Enter or
 * when its field loses focus, and a stepped size or a chosen family commits at once;
 * an invalid draft stays visible with its message and changes nothing, and a change the file refuses
 * puts the field back on the file's value with the reason.
 */
export function useAppearanceSettings(
	controller: TerminalSettingsController,
): AppearanceSettingsView {
	const intl = useOrchbeeIntl();
	const { preferences, presented } = useTerminalSettings(controller);
	const settings = controller.userSettings;
	const fontFamily = useSetting(settings, 'terminal.font_family');
	const fontSize = useSetting(settings, 'terminal.font_size');
	const foreground = useSetting(settings, 'appearance.custom_foreground');
	const background = useSetting(settings, 'appearance.custom_background');
	const fontSizeDraft = useSettingDraft(String(preferences.fontSize), fontSize.failure);
	const foregroundDraft = useSettingDraft(preferences.customForeground, foreground.failure);
	const backgroundDraft = useSettingDraft(preferences.customBackground, background.failure);
	const fontFamilyDraft = useSettingDraft(preferences.fontFamily, fontFamily.failure);
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

	const fontSizeInvalid = intl.formatMessage(
		terminalSettingsMessages.fontSizeRangeInvalid,
		TERMINAL_FONT_SIZE_RANGE,
	);
	const commitFontSize = (draft: string): void => {
		markInvalid(
			'terminal.font_size',
			controller.update({ fontSize: Number(draft) }).kind === 'invalid',
		);
	};
	const commitFontList = (fontList: string): void => {
		// A blur or Enter that changes nothing writes nothing.
		if (fontList === preferences.fontFamily) {
			markInvalid('terminal.font_family', false);
			return;
		}
		const result = controller.update({ fontFamily: fontList });
		markInvalid('terminal.font_family', result.kind === 'invalid');
		if (result.kind !== 'invalid') fontFamilyDraft.change(fontList);
	};
	const resetFontFamily = (): void => {
		markInvalid('terminal.font_family', false);
		fontFamilyDraft.change(BUILT_IN_TERMINAL_FONT_LIST);
		fontFamily.restore();
	};
	const commitFontFamily = (text: string): void => {
		const fontList = terminalFontListFromText(text);
		if (fontList === null) markInvalid('terminal.font_family', true);
		else if (fontList === '') resetFontFamily();
		else commitFontList(fontList);
	};
	const fontFamilyKeyDown = (event: React.KeyboardEvent<HTMLInputElement>): void => {
		// Escape takes back an edit the setting does not hold; with no edit it is left to close Settings.
		if (event.key === 'Escape' && fontFamilyDraft.draft !== preferences.fontFamily) {
			event.preventDefault();
			event.stopPropagation();
			markInvalid('terminal.font_family', false);
			fontFamilyDraft.change(preferences.fontFamily);
			return;
		}
		commitOnEnter(() => {
			commitFontFamily(fontFamilyDraft.draft);
		})(event);
	};

	return {
		background: colorDraft(
			background,
			'customBackground',
			backgroundDraft,
			intl.formatMessage(terminalSettingsMessages.backgroundInvalid),
		),
		fontFamily: {
			draft: fontFamilyDraft.draft,
			draftPreview: fontFamilyPreview(fontFamilyDraft.draft),
			error: invalid.has('terminal.font_family')
				? intl.formatMessage(terminalSettingsMessages.fontFamilyInvalid)
				: fontFamily.failureMessage,
			modified: fontFamily.modified,
			onBlur: () => {
				commitFontFamily(fontFamilyDraft.draft);
			},
			onChange: (event) => {
				fontFamilyDraft.change(event.currentTarget.value);
			},
			onChoose: (fontList) => {
				if (fontList === null) markInvalid('terminal.font_family', true);
				else commitFontList(fontList);
			},
			onChooseBuiltIn: resetFontFamily,
			onKeyDown: fontFamilyKeyDown,
			onRestore: fontFamily.readOnly ? undefined : resetFontFamily,
			readOnly: fontFamily.readOnly,
		},
		fontSize: {
			...textDraft(fontSize, 'terminal.font_size', fontSizeDraft, fontSizeInvalid, commitFontSize),
			onStep: (size) => {
				fontSizeDraft.change(String(size));
				commitFontSize(String(size));
			},
			value: preferences.fontSize,
		},
		foreground: colorDraft(
			foreground,
			'customForeground',
			foregroundDraft,
			intl.formatMessage(terminalSettingsMessages.foregroundInvalid),
		),
		preferences,
		presented,
	};
}

/** The list a font draft previews while it is admitted: the default for a blank draft; null otherwise. */
function fontFamilyPreview(text: string): string | null {
	const fontList = terminalFontListFromText(text);
	return fontList === '' ? BUILT_IN_TERMINAL_FONT_LIST : fontList;
}

function commitOnEnter(commit: () => void): React.KeyboardEventHandler<HTMLInputElement> {
	return (event) => {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		commit();
		event.currentTarget.blur();
	};
}
