import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import {
	formatKeyboardShortcutChord,
	matchesKeyboardShortcutChord,
	type KeyboardShortcutChord,
	type KeyboardShortcutPlatform,
} from '@orchbee/keyboard-shortcuts';
import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { SearchDraft } from '../../../../../../SearchSessions';

import styles from './SearchForm.module.css';

type Toggle = 'matchCase' | 'wholeWord' | 'regex';

/** Alt+C, Alt+W and Alt+R toggle the options, and Cmd+Option on a Mac, as an editor's do. */
function toggleShortcut(code: string, platform: KeyboardShortcutPlatform): KeyboardShortcutChord {
	return { code, modifiers: platform === 'apple' ? ['mod', 'alt'] : ['alt'] };
}

const TOGGLES = [
	{
		code: 'KeyC',
		icon: 'case-sensitive',
		key: 'matchCase',
		message: workbenchMessages.rightBarSearchMatchCase,
	},
	{
		code: 'KeyW',
		icon: 'whole-word',
		key: 'wholeWord',
		message: workbenchMessages.rightBarSearchWholeWord,
	},
	{ code: 'KeyR', icon: 'regex', key: 'regex', message: workbenchMessages.rightBarSearchRegex },
] as const satisfies readonly {
	readonly code: string;
	readonly icon: React.ComponentProps<typeof ProductIcon>['name'];
	readonly key: Toggle;
	readonly message: (typeof workbenchMessages)[keyof typeof workbenchMessages];
}[];

/** The draft with one option turned the other way. */
function flipped(draft: SearchDraft, key: Toggle): Partial<SearchDraft> {
	switch (key) {
		case 'matchCase':
			return { matchCase: !draft.matchCase };
		case 'wholeWord':
			return { wholeWord: !draft.wholeWord };
		case 'regex':
			return { regex: !draft.regex };
	}
}

/** Adds a line to the search text, which then matches across lines. */
const NEW_LINE_SHORTCUT = {
	code: 'Enter',
	modifiers: ['mod'],
} as const satisfies KeyboardShortcutChord;
/** Moves to the next or previous box, and from the last into the results. */
const NEXT_INPUT_SHORTCUT = {
	code: 'ArrowDown',
	modifiers: ['mod'],
} as const satisfies KeyboardShortcutChord;
const PREVIOUS_INPUT_SHORTCUT = {
	code: 'ArrowUp',
	modifiers: ['mod'],
} as const satisfies KeyboardShortcutChord;

export interface SearchFormProps {
	readonly draft: SearchDraft;
	/** Texts searched for, oldest first, which ↑ and ↓ bring back. */
	readonly history: readonly string[];
	readonly shortcutPlatform: KeyboardShortcutPlatform;
	readonly patternReference: React.RefObject<HTMLTextAreaElement | null>;
	readonly onChange: (change: Partial<SearchDraft>) => void;
	/** Searches now, without waiting for typing to pause. */
	readonly onRun: () => void;
	/** Moves the keyboard into the results; false when there are none. */
	readonly onFocusResults: () => boolean;
	/** Hands the keyboard back to the terminal. */
	readonly onEscape: () => void;
}

/**
 * The search view's boxes: the text with its three options, and, behind the details toggle, the
 * files to include and exclude with their own options. Enter searches at once; typing searches
 * once it pauses. ↑ on the first line and ↓ on the last walk through earlier searches, ↓ on the
 * last line otherwise moves into the results, and Mod+↑ and Mod+↓ move between the boxes.
 */
export function SearchForm({
	draft,
	history,
	shortcutPlatform,
	patternReference,
	onChange,
	onRun,
	onFocusResults,
	onEscape,
}: SearchFormProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const includesReference = React.useRef<HTMLInputElement | null>(null);
	const excludesReference = React.useRef<HTMLInputElement | null>(null);
	// Where ↑ and ↓ are in the history, and the text typed before walking into it.
	const [walk, setWalk] = React.useState<{ readonly index: number; readonly draft: string } | null>(
		null,
	);

	const boxes = (): readonly (HTMLTextAreaElement | HTMLInputElement)[] =>
		[patternReference.current, includesReference.current, excludesReference.current].flatMap(
			(box) => (box === null ? [] : [box]),
		);
	/** Moves the keyboard to the box after or before `from`, or past the last into the results. */
	const moveFrom = (from: HTMLTextAreaElement | HTMLInputElement, direction: 1 | -1): void => {
		const all = boxes();
		const next = all[all.indexOf(from) + direction];
		if (next !== undefined) next.focus();
		else if (direction === 1) onFocusResults();
	};

	/** The keys every box shares: the options, Enter, Esc and moving between boxes. */
	const sharedKeys = (
		event: React.KeyboardEvent<HTMLTextAreaElement | HTMLInputElement>,
	): boolean => {
		const native = event.nativeEvent;
		for (const toggle of TOGGLES) {
			if (
				matchesKeyboardShortcutChord(
					native,
					toggleShortcut(toggle.code, shortcutPlatform),
					shortcutPlatform,
				)
			) {
				onChange(flipped(draft, toggle.key));
				return true;
			}
		}
		if (matchesKeyboardShortcutChord(native, NEXT_INPUT_SHORTCUT, shortcutPlatform)) {
			moveFrom(event.currentTarget, 1);
			return true;
		}
		if (matchesKeyboardShortcutChord(native, PREVIOUS_INPUT_SHORTCUT, shortcutPlatform)) {
			moveFrom(event.currentTarget, -1);
			return true;
		}
		if (event.key === 'Escape') {
			onEscape();
			return true;
		}
		if (
			event.key === 'Enter' &&
			!event.nativeEvent.isComposing &&
			!event.altKey &&
			!event.ctrlKey &&
			!event.metaKey
		) {
			onRun();
			return true;
		}
		return false;
	};

	const recall = (index: number, typed: string): void => {
		setWalk(index === history.length ? null : { draft: typed, index });
		onChange({ pattern: index === history.length ? typed : (history[index] ?? typed) });
	};
	const onPatternKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>): void => {
		const box = event.currentTarget;
		if (matchesKeyboardShortcutChord(event.nativeEvent, NEW_LINE_SHORTCUT, shortcutPlatform)) {
			event.preventDefault();
			box.setRangeText('\n', box.selectionStart, box.selectionEnd, 'end');
			setWalk(null);
			onChange({ pattern: box.value });
			return;
		}
		if (sharedKeys(event)) {
			event.preventDefault();
			return;
		}
		if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
		if (box.selectionStart !== box.selectionEnd) return;
		const before = box.value.slice(0, box.selectionStart);
		const after = box.value.slice(box.selectionEnd);
		if (event.key === 'ArrowUp' && !before.includes('\n')) {
			// The text already in the box is skipped, as when it was the last one searched.
			let index = walk === null ? history.length - 1 : walk.index - 1;
			while (index >= 0 && history[index] === draft.pattern) index -= 1;
			if (index < 0) return;
			event.preventDefault();
			recall(index, walk?.draft ?? draft.pattern);
		} else if (event.key === 'ArrowDown' && !after.includes('\n')) {
			if (walk !== null) {
				event.preventDefault();
				recall(walk.index + 1, walk.draft);
			} else if (onFocusResults()) {
				event.preventDefault();
			}
		}
	};
	const onGlobKeyDown = (event: React.KeyboardEvent<HTMLInputElement>): void => {
		if (sharedKeys(event)) event.preventDefault();
	};

	/** One option beside a box: pressed while it applies. */
	const option = (
		label: string,
		icon: React.ComponentProps<typeof ProductIcon>['name'],
		pressed: boolean,
		onToggle: () => void,
	): React.ReactNode => (
		<button
			aria-label={label}
			aria-pressed={pressed}
			className={styles['option']}
			key={label}
			onClick={onToggle}
			title={label}
			type='button'
		>
			<ProductIcon name={icon} />
		</button>
	);

	const searchLabel = intl.formatMessage(workbenchMessages.rightBarSearchInput);
	const detailsLabel = intl.formatMessage(workbenchMessages.rightBarSearchDetails);
	return (
		<div className={styles['root']}>
			<div className={styles['box']}>
				<textarea
					aria-label={searchLabel}
					className={styles['pattern']}
					onChange={(event) => {
						// Text typed over a recalled one is the new draft.
						setWalk(null);
						onChange({ pattern: event.target.value });
					}}
					onKeyDown={onPatternKeyDown}
					placeholder={searchLabel}
					ref={patternReference}
					rows={1}
					spellCheck={false}
					title={intl.formatMessage(workbenchMessages.rightBarSearchInputTitle, {
						newLine: formatKeyboardShortcutChord(NEW_LINE_SHORTCUT, shortcutPlatform),
					})}
					value={draft.pattern}
				/>
				<div className={styles['options']}>
					{TOGGLES.map((toggle) => {
						const label = intl.formatMessage(toggle.message);
						const shortcut = formatKeyboardShortcutChord(
							toggleShortcut(toggle.code, shortcutPlatform),
							shortcutPlatform,
						);
						return option(`${label} (${shortcut})`, toggle.icon, draft[toggle.key], () =>
							onChange(flipped(draft, toggle.key)),
						);
					})}
				</div>
			</div>
			<div className={styles['detailsRow']}>
				<button
					aria-expanded={draft.details}
					aria-label={detailsLabel}
					className={styles['option']}
					onClick={() => onChange({ details: !draft.details })}
					title={detailsLabel}
					type='button'
				>
					<ProductIcon name='ellipsis' />
				</button>
			</div>
			{draft.details ? (
				<>
					<label className={styles['globLabel']}>
						{intl.formatMessage(workbenchMessages.rightBarSearchIncludes)}
						<span className={styles['box']}>
							<input
								className={styles['glob']}
								onChange={(event) => onChange({ includes: event.target.value })}
								onKeyDown={onGlobKeyDown}
								placeholder={intl.formatMessage(
									workbenchMessages.rightBarSearchIncludesPlaceholder,
								)}
								ref={includesReference}
								spellCheck={false}
								type='text'
								value={draft.includes}
							/>
							<span className={styles['options']}>
								{option(
									intl.formatMessage(workbenchMessages.rightBarSearchOnlyOpen),
									'book',
									draft.onlyOpen,
									() => onChange({ onlyOpen: !draft.onlyOpen }),
								)}
								{option(
									intl.formatMessage(workbenchMessages.rightBarSearchOnlyChanged),
									'diff',
									draft.onlyChanged,
									() => onChange({ onlyChanged: !draft.onlyChanged }),
								)}
							</span>
						</span>
					</label>
					<label className={styles['globLabel']}>
						{intl.formatMessage(workbenchMessages.rightBarSearchExcludes)}
						<span className={styles['box']}>
							<input
								className={styles['glob']}
								onChange={(event) => onChange({ excludes: event.target.value })}
								onKeyDown={onGlobKeyDown}
								placeholder={intl.formatMessage(
									workbenchMessages.rightBarSearchExcludesPlaceholder,
								)}
								ref={excludesReference}
								spellCheck={false}
								type='text'
								value={draft.excludes}
							/>
							<span className={styles['options']}>
								{option(
									intl.formatMessage(workbenchMessages.rightBarSearchUseIgnore),
									'exclude',
									draft.useIgnoreFiles,
									() => onChange({ useIgnoreFiles: !draft.useIgnoreFiles }),
								)}
							</span>
						</span>
					</label>
				</>
			) : null}
		</div>
	);
}
