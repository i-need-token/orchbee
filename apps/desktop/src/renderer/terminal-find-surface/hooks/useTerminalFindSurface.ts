import { desktopTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import type {
	DesktopTerminalFindController,
	DesktopTerminalFindNotice,
	DesktopTerminalFindSnapshot,
} from '../model/DesktopTerminalFindController';

interface UseTerminalFindSurfaceOptions {
	readonly controller: DesktopTerminalFindController;
	readonly onNext: () => void;
	readonly onPrevious: () => void;
	readonly restoreFocus: () => void;
}

interface TerminalFindSurfaceView {
	readonly caseSensitiveLabel: string;
	readonly closeLabel: string;
	readonly findLabel: string;
	readonly inputReference: React.RefObject<HTMLInputElement | null>;
	readonly nextLabel: string;
	readonly onCaseSensitiveChange: (pressed: boolean) => void;
	readonly onClose: () => void;
	readonly onInputChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
	readonly onInputKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => void;
	readonly onNext: () => void;
	readonly onPrevious: () => void;
	readonly onRegexChange: (pressed: boolean) => void;
	readonly onWholeWordChange: (pressed: boolean) => void;
	readonly placeholder: string;
	readonly previousLabel: string;
	readonly regexLabel: string;
	readonly snapshot: DesktopTerminalFindSnapshot;
	readonly statusText: string;
	readonly wholeWordLabel: string;
}

export function useTerminalFindSurface({
	controller,
	onNext,
	onPrevious,
	restoreFocus,
}: UseTerminalFindSurfaceOptions): TerminalFindSurfaceView {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(
		controller.subscribe,
		controller.getSnapshot,
		controller.getSnapshot,
	);
	const inputReference = React.useRef<HTMLInputElement>(null);
	const wasOpen = React.useRef(false);

	React.useEffect(() => {
		if (snapshot.isOpen) {
			inputReference.current?.focus({ preventScroll: true });
			inputReference.current?.select();
		} else if (wasOpen.current) {
			restoreFocus();
		}
		wasOpen.current = snapshot.isOpen;
	}, [restoreFocus, snapshot.focusRequest, snapshot.isOpen]);

	const invokeNext = (): void => {
		controller.recordCurrentQuery();
		onNext();
	};
	const invokePrevious = (): void => {
		controller.recordCurrentQuery();
		onPrevious();
	};
	const formatNotice = (notice: DesktopTerminalFindNotice): string => {
		switch (notice.kind) {
			case 'copy-completed':
				return intl.formatMessage(desktopTerminalMessages.copyCompleted, {
					byteLength: notice.byteLength,
				});
			case 'copy-failed':
				return intl.formatMessage(desktopTerminalMessages.copyFailed);
			case 'history-evicted':
				return intl.formatMessage(desktopTerminalMessages.historyEvicted);
			case 'history-unavailable':
				return intl.formatMessage(desktopTerminalMessages.historyUnavailable);
			case 'link-blocked':
				return intl.formatMessage(desktopTerminalMessages.linkBlocked);
			case 'link-failed':
				return intl.formatMessage(desktopTerminalMessages.linkFailed);
			case 'selection-evicted':
				return intl.formatMessage(desktopTerminalMessages.selectionEvicted);
		}
	};
	const formatStatus = (): string => {
		if (snapshot.notice !== null) return formatNotice(snapshot.notice);
		switch (snapshot.searchState) {
			case 'error':
				return intl.formatMessage(
					snapshot.searchErrorKind === 'invalid-query'
						? desktopTerminalMessages.findInvalidRegex
						: desktopTerminalMessages.findUnavailable,
				);
			case 'searching':
				return intl.formatMessage(desktopTerminalMessages.findSearching);
			case 'timeout':
				return intl.formatMessage(desktopTerminalMessages.findTimeout);
			case 'complete':
				if (snapshot.resultCount === 0) {
					return intl.formatMessage(desktopTerminalMessages.findNoResults);
				}
				if (snapshot.resultLimitReached) {
					return intl.formatMessage(desktopTerminalMessages.findResultLimit, {
						total: snapshot.resultCount,
					});
				}
				return intl.formatMessage(desktopTerminalMessages.findResult, {
					current: Math.max(1, snapshot.activeIndex + 1),
					total: snapshot.resultCount,
				});
			case 'idle':
				return intl.formatMessage(desktopTerminalMessages.findIdle);
		}
	};

	return {
		caseSensitiveLabel: intl.formatMessage(desktopTerminalMessages.matchCase),
		closeLabel: intl.formatMessage(desktopTerminalMessages.closeFind),
		findLabel: intl.formatMessage(desktopTerminalMessages.findLabel),
		inputReference,
		nextLabel: intl.formatMessage(desktopTerminalMessages.nextMatch),
		onCaseSensitiveChange: (pressed: boolean) => controller.setCaseSensitive(pressed),
		onClose: () => controller.close(),
		onInputChange: (event: React.ChangeEvent<HTMLInputElement>) =>
			controller.setQuery(event.currentTarget.value),
		onInputKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => {
			if (event.nativeEvent.isComposing) return;
			if (event.key === 'Escape') {
				event.preventDefault();
				event.stopPropagation();
				controller.close();
				return;
			}
			if (event.key === 'Enter') {
				event.preventDefault();
				event.stopPropagation();
				if (event.shiftKey) invokePrevious();
				else invokeNext();
				return;
			}
			if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
			if (event.key === 'ArrowUp' && controller.moveInQueryHistory(-1)) {
				event.preventDefault();
			}
			if (event.key === 'ArrowDown' && controller.moveInQueryHistory(1)) {
				event.preventDefault();
			}
		},
		onNext: invokeNext,
		onPrevious: invokePrevious,
		onRegexChange: (pressed: boolean) => controller.setRegex(pressed),
		onWholeWordChange: (pressed: boolean) => controller.setWholeWord(pressed),
		placeholder: intl.formatMessage(desktopTerminalMessages.findPlaceholder),
		previousLabel: intl.formatMessage(desktopTerminalMessages.previousMatch),
		regexLabel: intl.formatMessage(desktopTerminalMessages.useRegex),
		snapshot,
		statusText: formatStatus(),
		wholeWordLabel: intl.formatMessage(desktopTerminalMessages.wholeWord),
	};
}
