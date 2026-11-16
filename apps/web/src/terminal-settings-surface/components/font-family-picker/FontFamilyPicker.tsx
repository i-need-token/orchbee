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
	ComboboxValue,
	Switch,
} from '@orchbee/ui/components';
import * as React from 'react';

import {
	BUILT_IN_TERMINAL_FONT_FAMILY,
	leadingTerminalFontFamily,
	terminalFontFamilyList,
} from '../../../terminal-settings';
import type { TerminalSettingsFontCatalog } from '../../types/terminal-settings-surface';
import { useInstalledFontFamilies } from './hooks/useInstalledFontFamilies';
import { cssFamilyName } from './utils/font-family-classification';
import styles from './FontFamilyPicker.module.css';

interface FontFamilyPickerProps {
	readonly catalog: TerminalSettingsFontCatalog | undefined;
	readonly invalid: boolean;
	readonly label: string;
	/** False while the setting holds its built-in default. */
	readonly modified: boolean;
	readonly readOnly: boolean;
	/** The stored CSS font list. */
	readonly value: string;
	readonly onChoose: (fontList: string) => void;
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
 * The terminal font as a searchable list of installed families, each named in its own face.
 * Fixed-width families come first and alone until the person shows all fonts; a search looks
 * through every family and also takes a family or a CSS font list the list lacks, and the built-in
 * font restores the default.
 */
export function FontFamilyPicker({
	catalog,
	invalid,
	label,
	modified,
	readOnly,
	value,
	onChoose,
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
				value: terminalFontFamilyList(family.family),
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
	const selected: FontEntry = !modified
		? builtIn
		: (groups.flatMap((group) => group.items).find((entry) => entry.value === value) ?? {
				kind: 'typed',
				label: value,
				value,
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
			onOpenChange={(open) => {
				if (open) installed.list();
				else setQuery('');
			}}
			onValueChange={(entry: FontEntry | null) => {
				if (entry === null || (entry.kind === selected.kind && entry.value === selected.value)) {
					return;
				}
				if (entry.kind === 'built-in') onRestore();
				else onChoose(entry.value);
			}}
		>
			<ComboboxTrigger aria-invalid={invalid || undefined} aria-label={label} size='sm'>
				<ComboboxValue>
					{modified ? leadingTerminalFontFamily(value) || value : builtIn.label}
				</ComboboxValue>
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

function sampleFamily(entry: FontEntry): string | null {
	if (entry.kind === 'built-in') return BUILT_IN_TERMINAL_FONT_FAMILY;
	if (entry.kind === 'family' && entry.sample) return entry.label;
	return null;
}
