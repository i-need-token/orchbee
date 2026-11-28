import { fileExplorerMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import {
	SCM_STATUS_MESSAGES,
	type ScmFileDecoration,
	type ScmStatusColor,
} from '../../../../source-control';
import type { ExplorerGitDecorations } from '../../../model/ExplorerGitDecorations';
import type { ExplorerItem } from '../../../model/explorer-item';

/** What sits at a row's far end: its letters, or the dot that stands for changes below a folder. */
type ExplorerRowBadge =
	| { readonly kind: 'descendants' }
	| { readonly kind: 'letters'; readonly text: string };

/** A row's decorations, merged. */
export interface ExplorerRowDecoration {
	readonly badge: ExplorerRowBadge | null;
	/** The color of the row's name and badge; null leaves them their own. */
	readonly color: ScmStatusColor | null;
	/** The badge's accessible name and tooltip. */
	readonly tooltip: string;
}

/** One decoration a row gathers. */
interface DecorationPart {
	readonly color: ScmStatusColor | null;
	readonly letter: string;
	readonly tooltip: string;
}

/** Git's gray for a path it ignores, which has no letter or hover text, as VS Code gives it. */
const IGNORED_PART: DecorationPart = { color: 'ignored', letter: '', tooltip: '' };

function subscribeToNothing(): () => void {
	return () => undefined;
}

/**
 * Merges a row's decorations as VS Code's decorations service does, Git's ahead of the Explorer's
 * own: the first color colors the name and the badge; changes below a folder turn the badge into a
 * dot that says so; otherwise the letters join with commas and the distinct hover texts with
 * bullets.
 */
function mergeDecorations(
	git: readonly DecorationPart[],
	descendant: DecorationPart | null,
	explorer: DecorationPart | null,
	descendantsTooltip: string,
): ExplorerRowDecoration | null {
	const parts = [...git];
	if (descendant !== null) parts.push(descendant);
	if (explorer !== null) parts.push(explorer);
	if (parts.length === 0) return null;
	const color = parts.find((part) => part.color !== null)?.color ?? null;
	if (descendant !== null) {
		return { badge: { kind: 'descendants' }, color, tooltip: descendantsTooltip };
	}
	const letters = parts.map((part) => part.letter).filter((letter) => letter !== '');
	const tooltips = new Set(parts.map((part) => part.tooltip).filter((tooltip) => tooltip !== ''));
	return {
		badge: letters.length === 0 ? null : { kind: 'letters', text: letters.join(', ') },
		color,
		tooltip: [...tooltips].join(' • '),
	};
}

/**
 * A row's decorations as VS Code's Explorer draws them: Git's, from the session's decorations,
 * which the row follows and tells that it shows, so Git is asked whether it ignores the path, and
 * the Explorer's own mark for a link or for an entry that is neither a file nor a folder.
 */
export function useExplorerRowDecoration(
	decorations: ExplorerGitDecorations | null,
	item: ExplorerItem,
): ExplorerRowDecoration | null {
	const intl = useOrchbeeIntl();
	React.useSyncExternalStore(
		decorations?.subscribe ?? subscribeToNothing,
		() => decorations?.version ?? 0,
	);
	React.useEffect(() => decorations?.show(item.path), [decorations, item.path]);
	const git = decorations?.decorationOf(item.path, item.directory) ?? null;
	const partOf = (decoration: ScmFileDecoration): DecorationPart => ({
		color: decoration.color,
		letter: decoration.letter,
		tooltip: intl.formatMessage(SCM_STATUS_MESSAGES[decoration.status]),
	});
	let explorer: DecorationPart | null = null;
	if (item.entry?.symlink === true) {
		const tooltip = intl.formatMessage(fileExplorerMessages.symbolicLink);
		explorer = { color: null, letter: '⤷', tooltip };
	} else if (item.entry?.type === 'unknown') {
		const tooltip = intl.formatMessage(fileExplorerMessages.unknownType);
		explorer = { color: null, letter: '?', tooltip };
	}
	const own: DecorationPart[] = [];
	if (git !== null && git.own !== null) own.push(partOf(git.own));
	if (git?.ignored === true) own.push(IGNORED_PART);
	return mergeDecorations(
		own,
		git === null || git.descendant === null ? null : partOf(git.descendant),
		explorer,
		intl.formatMessage(fileExplorerMessages.containsEmphasizedItems),
	);
}
