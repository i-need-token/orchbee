import { fileBarMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import type { TreeFind, TreeFindSnapshot } from '../../../find/TreeFind';
import { treeKeyCommands } from '../../../interaction/tree-keys';

interface VirtualTreeFindView {
	readonly snapshot: TreeFindSnapshot;
	readonly inputReference: React.RefObject<HTMLInputElement | null>;
	/** The message element's id, which the input names as its description while it shows. */
	readonly messageId: string;
	readonly label: string;
	readonly inputLabel: string;
	readonly placeholder: string;
	readonly filterLabel: string;
	readonly fuzzyMatchLabel: string;
	readonly closeLabel: string;
	readonly messageText: string | null;
	/** What the polite live region says after a pattern applies; null before anything applied. */
	readonly announcementText: string | null;
	readonly onInputChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
	readonly onInputKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => void;
	readonly onKeyDown: (event: React.KeyboardEvent<HTMLElement>) => void;
	readonly onFilterChange: (pressed: boolean, event: Event) => void;
	readonly onFuzzyMatchChange: (pressed: boolean, event: Event) => void;
	readonly onClose: () => void;
}

/**
 * The find band's presenter: VS Code's tree find widget over a `TreeFind`. The input takes focus with
 * its text selected whenever the find opens or is asked for again. Enter and Down past the newest
 * remembered pattern return to the tree, Up and Down walk the remembered patterns, and anywhere in
 * the band Escape closes the find and the tree's find keys select the input's text again. A click on
 * a toggle leaves focus in the input, as VS Code's find input does.
 */
export function useVirtualTreeFind<K, T>(
	find: TreeFind<K, T>,
	platform: KeyboardShortcutPlatform,
): VirtualTreeFindView {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(find.subscribe, find.getSnapshot, find.getSnapshot);
	const inputReference = React.useRef<HTMLInputElement>(null);
	const messageId = React.useId();
	React.useEffect(() => {
		if (!snapshot.open) return;
		inputReference.current?.focus({ preventScroll: true });
		inputReference.current?.select();
	}, [snapshot.focusRequest, snapshot.open]);

	const toggle = (apply: () => void, event: Event): void => {
		apply();
		if (event instanceof MouseEvent && event.detail > 0) {
			inputReference.current?.focus({ preventScroll: true });
		}
	};
	const { announcement, message, mode } = snapshot;
	let messageText: string | null = null;
	if (message?.kind === 'notFound') messageText = intl.formatMessage(fileBarMessages.findNotFound);
	else if (message?.kind === 'warning') messageText = message.text;
	let announcementText: string | null = null;
	if (announcement !== null) {
		const results = intl.formatMessage(fileBarMessages.findResults, { count: announcement.count });
		// A provider's warning is said with the count, since its message only shows.
		announcementText = message?.kind === 'warning' ? `${results} ${message.text}` : results;
	}
	const searchPlaceholder = intl.formatMessage(fileBarMessages.findSearchPlaceholder);

	return {
		announcementText,
		closeLabel: intl.formatMessage(fileBarMessages.findClose),
		filterLabel: intl.formatMessage(fileBarMessages.findFilterMode),
		fuzzyMatchLabel: intl.formatMessage(fileBarMessages.findFuzzyMatch),
		inputLabel: searchPlaceholder,
		inputReference,
		label: intl.formatMessage(fileBarMessages.findLabel),
		messageId,
		messageText,
		onClose: () => {
			find.close();
		},
		onFilterChange: (pressed: boolean, event: Event) => {
			toggle(() => find.setMode(pressed ? 'filter' : 'highlight'), event);
		},
		onFuzzyMatchChange: (pressed: boolean, event: Event) => {
			toggle(() => find.setMatchType(pressed ? 'fuzzy' : 'contiguous'), event);
		},
		onInputChange: (event: React.ChangeEvent<HTMLInputElement>) => {
			find.type(event.currentTarget.value);
		},
		onInputKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => {
			if (event.nativeEvent.isComposing) return;
			if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
			switch (event.key) {
				case 'Enter':
					find.commit();
					break;
				case 'ArrowDown':
					find.historyNext();
					break;
				case 'ArrowUp':
					find.historyPrevious();
					break;
				default:
					return;
			}
			event.preventDefault();
			event.stopPropagation();
		},
		onKeyDown: (event: React.KeyboardEvent<HTMLElement>) => {
			if (event.nativeEvent.isComposing) return;
			const commands = treeKeyCommands(event.nativeEvent, platform);
			if (commands.includes('find')) find.open();
			else if (commands.includes('closeFind')) find.close();
			else return;
			event.preventDefault();
			event.stopPropagation();
		},
		placeholder:
			mode === 'filter'
				? intl.formatMessage(fileBarMessages.findFilterPlaceholder)
				: searchPlaceholder,
		snapshot,
	};
}
