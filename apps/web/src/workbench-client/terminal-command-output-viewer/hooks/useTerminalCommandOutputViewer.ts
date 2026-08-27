import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import {
	projectTerminalCommandOutputSegments,
	type TerminalCommandOutputSegment,
	type TerminalCommandOutputViewerController,
	type TerminalCommandOutputViewerSnapshot,
} from '../model/terminal-command-output-viewer-controller';

interface UseTerminalCommandOutputViewerOptions {
	readonly controller: TerminalCommandOutputViewerController;
	readonly platform: KeyboardShortcutPlatform;
	readonly restoreFocus: () => void;
}

interface TerminalCommandOutputViewerView {
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
	readonly segments: readonly TerminalCommandOutputSegment[];
	readonly snapshot: TerminalCommandOutputViewerSnapshot;
	readonly statusText: string;
	readonly statusId: string;
	readonly title: string;
	readonly titleId: string;
	readonly truncatedText: string | null;
}

export function useTerminalCommandOutputViewer({
	controller,
	platform,
	restoreFocus,
}: UseTerminalCommandOutputViewerOptions): TerminalCommandOutputViewerView {
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
			document?.kind === 'output'
				? projectTerminalCommandOutputSegments(
						document.text,
						snapshot.matches,
						snapshot.activeMatchIndex,
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
		emptyText = intl.formatMessage(
			document.reason === 'no-shell-integration'
				? projectedTerminalMessages.outputViewerNoShellIntegration
				: projectedTerminalMessages.outputViewerNoOutput,
		);
	} else if (document?.kind === 'unavailable') {
		if (document.reason === 'alternate-screen') {
			emptyText = intl.formatMessage(projectedTerminalMessages.outputViewerAlternateScreen);
		} else if (document.reason === 'source-limit') {
			emptyText = intl.formatMessage(projectedTerminalMessages.outputViewerSourceLimit);
		} else if (document.reason === 'stale') {
			emptyText = intl.formatMessage(projectedTerminalMessages.outputViewerStale);
		} else {
			emptyText = intl.formatMessage(projectedTerminalMessages.outputViewerUnavailable);
		}
	}

	let statusText = '';
	if (snapshot.query !== '') {
		statusText =
			snapshot.matches.length === 0
				? intl.formatMessage(projectedTerminalMessages.outputViewerNoMatches)
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
		closeLabel: intl.formatMessage(projectedTerminalMessages.outputViewerClose),
		contentLabel: intl.formatMessage(projectedTerminalMessages.outputViewerContent),
		contentReference,
		copyLabel: intl.formatMessage(projectedTerminalMessages.outputCopyAction),
		emptyText,
		inputReference,
		nextLabel: intl.formatMessage(projectedTerminalMessages.findNextAction),
		onClose: close,
		onKeyDown,
		onQueryChange: (event) => controller.setQuery(event.currentTarget.value),
		previousLabel: intl.formatMessage(projectedTerminalMessages.findPreviousAction),
		rootReference,
		searchLabel: intl.formatMessage(projectedTerminalMessages.outputViewerSearch),
		segments,
		snapshot,
		statusText,
		statusId,
		title: intl.formatMessage(projectedTerminalMessages.outputViewerTitle),
		titleId,
		truncatedText:
			document?.kind === 'output' && document.truncatedStart
				? intl.formatMessage(projectedTerminalMessages.outputViewerTruncated)
				: null,
	};
}
