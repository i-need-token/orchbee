import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import {
	projectTerminalTextViewerSegments,
	type TerminalTextViewerController,
	type TerminalTextViewerSegment,
	type TerminalTextViewerSnapshot,
} from '../model/terminal-text-viewer-controller';

interface UseTerminalTextViewerOptions {
	readonly controller: TerminalTextViewerController;
	readonly platform: KeyboardShortcutPlatform;
	readonly restoreFocus: () => void;
}

interface TerminalTextViewerView {
	readonly closeLabel: string;
	readonly contentLabel: string;
	readonly contentReference: React.RefObject<HTMLDivElement | null>;
	readonly copyLabel: string;
	readonly emptyText: string | null;
	readonly inputReference: React.RefObject<HTMLInputElement | null>;
	readonly nextLabel: string;
	readonly onClose: () => void;
	readonly onKeyDown: (event: React.KeyboardEvent<HTMLElement>) => void;
	readonly onQueryChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
	readonly previousLabel: string;
	readonly rootReference: React.RefObject<HTMLElement | null>;
	readonly searchLabel: string;
	readonly segments: readonly TerminalTextViewerSegment[];
	readonly snapshot: TerminalTextViewerSnapshot;
	readonly statusText: string;
	readonly statusId: string;
	readonly title: string;
	readonly titleId: string;
	readonly truncatedText: string | null;
}

const unavailableViewerMessages = {
	'command-output': {
		'alternate-screen': projectedTerminalMessages.outputViewerAlternateScreen,
		'source-limit': projectedTerminalMessages.outputViewerSourceLimit,
		stale: projectedTerminalMessages.outputViewerStale,
		unavailable: projectedTerminalMessages.outputViewerUnavailable,
	},
	scrollback: {
		'alternate-screen': projectedTerminalMessages.scrollbackViewerAlternateScreen,
		'source-limit': projectedTerminalMessages.scrollbackViewerSourceLimit,
		stale: projectedTerminalMessages.scrollbackViewerStale,
		unavailable: projectedTerminalMessages.scrollbackViewerUnavailable,
	},
} as const;

export function useTerminalTextViewer({
	controller,
	platform,
	restoreFocus,
}: UseTerminalTextViewerOptions): TerminalTextViewerView {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(
		controller.subscribe,
		controller.getSnapshot,
		controller.getSnapshot,
	);
	const inputReference = React.useRef<HTMLInputElement>(null);
	const contentReference = React.useRef<HTMLDivElement>(null);
	const rootReference = React.useRef<HTMLElement>(null);
	const statusId = React.useId();
	const titleId = React.useId();
	const document = snapshot.document;
	const segments = React.useMemo(
		() =>
			document?.kind === 'text'
				? projectTerminalTextViewerSegments(
						document.text,
						snapshot.matches,
						snapshot.activeMatchIndex,
						document.initialTopOffset,
					)
				: [],
		[document, snapshot.activeMatchIndex, snapshot.matches],
	);

	React.useEffect(() => {
		if (!snapshot.isOpen) return;
		const input = inputReference.current;
		if (input === null) {
			rootReference.current?.querySelector<HTMLElement>('button:not(:disabled)')?.focus({
				preventScroll: true,
			});
			return;
		}
		input.focus({ preventScroll: true });
		input.select();
	}, [snapshot.focusRequest, snapshot.isOpen]);

	React.useLayoutEffect(() => {
		if (document?.kind !== 'text' || document.source !== 'scrollback') return;
		const anchor = contentReference.current?.querySelector<HTMLElement>(
			'[data-initial-scrollback-anchor="true"]',
		);
		anchor?.scrollIntoView?.({ block: 'start' });
	}, [document]);

	React.useEffect(() => {
		if (snapshot.activeMatchIndex < 0) return;
		const active = contentReference.current?.querySelector<HTMLElement>('[data-active="true"]');
		active?.scrollIntoView?.({ block: 'center' });
	}, [snapshot.activeMatchIndex, snapshot.matches]);

	const close = (): void => {
		controller.close();
		restoreFocus();
	};
	const onKeyDown = (event: React.KeyboardEvent<HTMLElement>): void => {
		if (event.key === 'Tab' && !event.altKey && !event.ctrlKey && !event.metaKey) {
			const input = inputReference.current;
			const content = contentReference.current;
			if (event.shiftKey && event.target === input && content !== null) {
				event.preventDefault();
				content.focus();
				return;
			}
			if (!event.shiftKey && event.target === content && input !== null) {
				event.preventDefault();
				input.focus();
				return;
			}
			if (input === null && content === null) {
				event.preventDefault();
				rootReference.current?.querySelector<HTMLElement>('button:not(:disabled)')?.focus();
				return;
			}
		}
		if (event.key === 'Escape' && !event.altKey && !event.ctrlKey && !event.metaKey) {
			event.preventDefault();
			event.stopPropagation();
			close();
			return;
		}
		const primaryModifier =
			platform === 'apple' ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
		if (event.code === 'KeyF' && primaryModifier && !event.altKey && !event.shiftKey) {
			event.preventDefault();
			event.stopPropagation();
			controller.focusQuery();
			return;
		}
		if (
			event.key === 'Enter' &&
			!event.altKey &&
			!event.ctrlKey &&
			!event.metaKey &&
			snapshot.query !== ''
		) {
			event.preventDefault();
			event.stopPropagation();
			controller.moveMatch(event.shiftKey ? -1 : 1);
		}
	};

	let emptyText: string | null = null;
	if (document?.kind === 'empty') {
		if (document.reason === 'no-shell-integration') {
			emptyText = intl.formatMessage(projectedTerminalMessages.outputViewerNoShellIntegration);
		} else if (document.reason === 'no-output') {
			emptyText = intl.formatMessage(projectedTerminalMessages.outputViewerNoOutput);
		} else {
			emptyText = intl.formatMessage(projectedTerminalMessages.scrollbackViewerNoScrollback);
		}
	} else if (document?.kind === 'unavailable') {
		emptyText = intl.formatMessage(unavailableViewerMessages[document.source][document.reason]);
	}

	let statusText = '';
	if (snapshot.query !== '') {
		statusText =
			snapshot.matches.length === 0
				? intl.formatMessage(
						document?.source === 'scrollback'
							? projectedTerminalMessages.scrollbackViewerNoMatches
							: projectedTerminalMessages.outputViewerNoMatches,
					)
				: intl.formatMessage(projectedTerminalMessages.findResult, {
						current: snapshot.activeMatchIndex + 1,
						total: snapshot.matches.length,
					});
		if (snapshot.matchLimitReached) {
			statusText = intl.formatMessage(projectedTerminalMessages.findResultLimit, {
				total: snapshot.matches.length,
			});
		}
	}

	return {
		closeLabel: intl.formatMessage(
			document?.source === 'scrollback'
				? projectedTerminalMessages.scrollbackViewerClose
				: projectedTerminalMessages.outputViewerClose,
		),
		contentLabel: intl.formatMessage(
			document?.source === 'scrollback'
				? projectedTerminalMessages.scrollbackViewerContent
				: projectedTerminalMessages.outputViewerContent,
		),
		contentReference,
		copyLabel: intl.formatMessage(
			document?.source === 'scrollback'
				? projectedTerminalMessages.scrollbackCopyAction
				: projectedTerminalMessages.outputCopyAction,
		),
		emptyText,
		inputReference,
		nextLabel: intl.formatMessage(projectedTerminalMessages.findNextAction),
		onClose: close,
		onKeyDown,
		onQueryChange: (event) => controller.setQuery(event.currentTarget.value),
		previousLabel: intl.formatMessage(projectedTerminalMessages.findPreviousAction),
		rootReference,
		searchLabel: intl.formatMessage(
			document?.source === 'scrollback'
				? projectedTerminalMessages.scrollbackViewerSearch
				: projectedTerminalMessages.outputViewerSearch,
		),
		segments,
		snapshot,
		statusText,
		statusId,
		title: intl.formatMessage(
			document?.source === 'scrollback'
				? projectedTerminalMessages.scrollbackViewerTitle
				: projectedTerminalMessages.outputViewerTitle,
		),
		titleId,
		truncatedText:
			document?.kind === 'text' && document.notice === 'truncated-start'
				? intl.formatMessage(projectedTerminalMessages.outputViewerTruncated)
				: null,
	};
}
