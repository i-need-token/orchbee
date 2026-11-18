import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	Combobox,
	ComboboxCollection,
	ComboboxContent,
	ComboboxGroup,
	ComboboxItem,
	ComboboxLabel,
	ComboboxList,
	ComboboxSearch,
	ComboboxTrigger,
	Input,
	Switch,
} from '@orchbee/ui/components';
import * as React from 'react';

import {
	BUILT_IN_TERMINAL_FONT_FAMILY,
	BUILT_IN_TERMINAL_FONT_LIST,
	leadingTerminalFontFamily,
	terminalFontListFromText,
	terminalFontListWithFamilyFirst,
} from '../../../terminal-settings';
import type { TerminalSettingsFontCatalog } from '../../types/terminal-settings-surface';
import { useInstalledFontFamilies } from './hooks/useInstalledFontFamilies';
import { cssFamilyName } from './utils/font-family-classification';
import styles from './FontFamilyPicker.module.css';

interface FontFamilyPickerProps {
	readonly catalog: TerminalSettingsFontCatalog | undefined;
	readonly invalid: boolean;
	readonly label: string;
	readonly readOnly: boolean;
	/** The font list the field shows, which the person may still be editing. */
	readonly value: string;
	readonly onBlur: () => void;
	readonly onChange: React.ChangeEventHandler<HTMLInputElement>;
	/** The font list a picked entry stores; null when that list does not parse or is too long. */
	readonly onChoose: (fontList: string | null) => void;
	readonly onKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	/** The font list the highlighted entry would store while the list is open; null once it closes or when that list is not admitted. */
	readonly onPreview: (fontList: string | null) => void;
	readonly onRestore: () => void;
}

type FontEntry =
	| { readonly kind: 'built-in'; readonly label: string; readonly value: string }
	| {
			readonly kind: 'family';
			readonly label: string;
			/** Whether the entry draws its name in its own face. */
			readonly sample: boolean;
			readonly value: string;
	  }
	| { readonly kind: 'typed'; readonly label: string; readonly value: string };

interface FontGroup {
	readonly id: 'fixed-width' | 'other' | 'typed';
	readonly items: readonly FontEntry[];
	readonly label: string | null;
}

/**
 * The terminal font as a field that takes a font list, with a Browse button that opens the installed
 * families beside it. The list is searchable, fixed-width families come first and alone until the person
 * shows all fonts, a search also takes a family or a CSS font list the list lacks, and the built-in font
 * restores the default. A picked family moves to the front of the field's list.
 */
export function FontFamilyPicker({
	catalog,
	invalid,
	label,
	readOnly,
	value,
	onBlur,
	onChange,
	onChoose,
	onKeyDown,
	onPreview,
	onRestore,
}: FontFamilyPickerProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const installed = useInstalledFontFamilies(catalog);
	const [query, setQuery] = React.useState('');
	const [showAll, setShowAll] = React.useState(false);
	const showAllId = React.useId();
	const builtIn: FontEntry = {
		kind: 'built-in',
		label: intl.formatMessage(terminalSettingsMessages.pageFontBuiltIn),
		value: '',
	};
	const familyEntries = (fixedWidth: boolean): FontEntry[] =>
		installed.families
			.filter((family) => (family.fixedWidth && !family.symbol) === fixedWidth)
			.map((family) => ({
				kind: 'family',
				label: family.family,
				sample: !family.symbol,
				value: family.family,
			}));
	const typed = query.trim();
	const typedIsListed = installed.families.some(
		(family) => family.family.toLocaleLowerCase() === typed.toLocaleLowerCase(),
	);
	const hasOthers = installed.families.some((family) => !family.fixedWidth || family.symbol);
	// A search looks through every installed family; the switch only widens the list to browse.
	const othersShown = hasOthers && (showAll || typed !== '');
	const groups: FontGroup[] = [
		{
			id: 'fixed-width',
			items: [builtIn, ...familyEntries(true)],
			label: othersShown
				? intl.formatMessage(terminalSettingsMessages.pageFontGroupFixedWidth)
				: null,
		},
		...(othersShown
			? [
					{
						id: 'other' as const,
						items: familyEntries(false),
						label: intl.formatMessage(terminalSettingsMessages.pageFontGroupOther),
					},
				]
			: []),
		...(typed === '' || typedIsListed
			? []
			: [
					{
						id: 'typed' as const,
						items: [
							{
								kind: 'typed' as const,
								label: intl.formatMessage(terminalSettingsMessages.pageFontUse, { family: typed }),
								value: typed,
							},
						],
						label: null,
					},
				]),
	];
	// A blank field is the default, so the built-in entry is the one its list leads with.
	const leading =
		value.trim() === '' ? BUILT_IN_TERMINAL_FONT_FAMILY : leadingTerminalFontFamily(value);
	const selected: FontEntry =
		leading === BUILT_IN_TERMINAL_FONT_FAMILY
			? builtIn
			: (groups.flatMap((group) => group.items).find((entry) => entry.value === leading) ?? {
					kind: 'typed',
					label: leading,
					value: leading,
				});

	return (
		<Combobox
			autoHighlight
			disabled={readOnly}
			inputValue={query}
			isItemEqualToValue={(item: FontEntry, current: FontEntry) =>
				item.kind === current.kind && item.value === current.value
			}
			itemToStringLabel={(item: FontEntry) => item.label}
			items={groups}
			value={selected}
			onInputValueChange={setQuery}
			onItemHighlighted={(entry: FontEntry | undefined) => {
				onPreview(entry === undefined ? null : storedFontList(entry, value));
			}}
			onOpenChange={(open) => {
				if (open) {
					installed.list();
					return;
				}
				setQuery('');
				onPreview(null);
			}}
			onValueChange={(entry: FontEntry | null) => {
				if (entry === null || (entry.kind === selected.kind && entry.value === selected.value)) {
					return;
				}
				if (entry.kind === 'built-in') onRestore();
				else onChoose(storedFontList(entry, value));
			}}
		>
			<Input
				aria-invalid={invalid || undefined}
				aria-label={label}
				className={styles['input']}
				readOnly={readOnly}
				spellCheck={false}
				value={value}
				onBlur={onBlur}
				onChange={onChange}
				onKeyDown={onKeyDown}
			/>
			<ComboboxTrigger size='sm'>
				{intl.formatMessage(terminalSettingsMessages.pageFontBrowse)}
			</ComboboxTrigger>
			<ComboboxContent align='end' className={styles['popup']}>
				<ComboboxSearch
					aria-label={label}
					placeholder={intl.formatMessage(terminalSettingsMessages.pageFontSearch)}
					spellCheck={false}
				/>
				<ComboboxList>
					{(group: FontGroup) => (
						<ComboboxGroup key={group.id} items={group.items}>
							{group.label === null ? null : <ComboboxLabel>{group.label}</ComboboxLabel>}
							<ComboboxCollection>
								{(entry: FontEntry) => (
									<ComboboxItem key={`${entry.kind}:${entry.value}`} value={entry}>
										<FontEntryName entry={entry} />
									</ComboboxItem>
								)}
							</ComboboxCollection>
						</ComboboxGroup>
					)}
				</ComboboxList>
				{hasOthers ? (
					<div className={styles['footer']}>
						<label className={styles['showAll']} htmlFor={showAllId}>
							{intl.formatMessage(terminalSettingsMessages.pageFontShowAll)}
						</label>
						<Switch
							checked={showAll}
							id={showAllId}
							size='sm'
							onCheckedChange={(checked): void => {
								setShowAll(checked);
							}}
						/>
					</div>
				) : null}
			</ComboboxContent>
		</Combobox>
	);
}

/**
 * An entry's name, drawn in its own face unless its face puts pictures where letters belong. The
 * face is the user's data, so it is drawn as SVG text with the family as a presentation attribute,
 * as the theme swatches draw a theme's colors; the hidden label carries the name for assistive
 * technology.
 */
function FontEntryName({ entry }: { readonly entry: FontEntry }): React.ReactNode {
	const family = sampleFamily(entry);
	if (family === null) return <span className={styles['name']}>{entry.label}</span>;
	return (
		<>
			<svg aria-hidden='true' className={styles['sample']}>
				<text
					dominantBaseline='central'
					fill='currentColor'
					fontFamily={`${cssFamilyName(family)}, sans-serif`}
					x='0'
					y='50%'
				>
					{entry.label}
				</text>
			</svg>
			<span className={styles['hiddenLabel']}>{entry.label}</span>
		</>
	);
}

/**
 * The font list an entry stores over the field's list, or null when that list does not parse or does
 * not fit the setting. A typed name with a comma is a font list the person means as it stands.
 */
function storedFontList(entry: FontEntry, fieldText: string): string | null {
	if (entry.kind === 'built-in') return BUILT_IN_TERMINAL_FONT_LIST;
	const list =
		entry.kind === 'typed' && entry.value.includes(',')
			? entry.value
			: terminalFontListWithFamilyFirst(fieldText, entry.value);
	return list === null ? null : terminalFontListFromText(list);
}

function sampleFamily(entry: FontEntry): string | null {
	if (entry.kind === 'built-in') return BUILT_IN_TERMINAL_FONT_FAMILY;
	if (entry.kind === 'family' && entry.sample) return entry.label;
	return null;
}
