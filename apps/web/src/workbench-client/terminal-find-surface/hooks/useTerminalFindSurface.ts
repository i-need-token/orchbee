import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import type {
	TerminalFindController,
	TerminalFindNotice,
	TerminalFindSnapshot,
} from '../model/TerminalFindController';

interface UseTerminalFindSurfaceOptions {
	readonly controller: TerminalFindController;
	readonly onNext: () => void;
	readonly onPrevious: () => void;
	readonly restoreFocus: () => void;
}

interface TerminalFindSurfaceView {
	readonly caseSensitiveLabel: string;
	readonly closeLabel: string;
	readonly compactStatusText: string | null;
	readonly findLabel: string;
	readonly inputReference: React.RefObject<HTMLInputElement | null>;
	readonly messageStatusText: string | null;
	readonly nextLabel: string;
	readonly onCaseSensitiveChange: (pressed: boolean, event: Event) => void;
	readonly onClose: () => void;
	readonly onInputChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
	readonly onInputKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => void;
	readonly onNext: () => void;
	readonly onOptionControlsKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
	readonly onPrevious: () => void;
	readonly onRegexChange: (pressed: boolean, event: Event) => void;
	readonly onSurfaceKeyDown: (event: React.KeyboardEvent<HTMLElement>) => void;
	readonly onWholeWordChange: (pressed: boolean, event: Event) => void;
	readonly placeholder: string;
	readonly previousLabel: string;
	readonly regexLabel: string;
	readonly snapshot: TerminalFindSnapshot;
	readonly statusId: string;
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
	const statusId = React.useId();

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
	const updateOption = (
		update: (pressed: boolean) => void,
		pressed: boolean,
		event: Event,
	): void => {
		update(pressed);
		if (event instanceof MouseEvent && event.detail > 0) {
			inputReference.current?.focus({ preventScroll: true });
		}
	};
	const formatNotice = (notice: TerminalFindNotice): string => {
		switch (notice.kind) {
			case 'history-evicted':
				return intl.formatMessage(projectedTerminalMessages.historyEvicted);
			case 'history-unavailable':
				return intl.formatMessage(projectedTerminalMessages.historyUnavailable);
			case 'link-blocked':
				return intl.formatMessage(projectedTerminalMessages.linkBlocked);
			case 'link-failed':
				return intl.formatMessage(projectedTerminalMessages.linkFailed);
			case 'selection-evicted':
				return intl.formatMessage(projectedTerminalMessages.selectionEvicted);
		}
	};
	const formatStatus = (): string => {
		if (snapshot.notice !== null) return formatNotice(snapshot.notice);
		switch (snapshot.searchState) {
			case 'error':
				return intl.formatMessage(
					snapshot.searchErrorKind === 'invalid-query'
						? projectedTerminalMessages.findInvalidRegex
						: projectedTerminalMessages.findUnavailable,
				);
			case 'searching':
				return intl.formatMessage(projectedTerminalMessages.findSearching);
			case 'timeout':
				return intl.formatMessage(projectedTerminalMessages.findTimeout);
			case 'complete':
				if (snapshot.resultCount === 0) {
					return intl.formatMessage(projectedTerminalMessages.findNoResults);
				}
				if (snapshot.resultLimitReached) {
					return intl.formatMessage(projectedTerminalMessages.findResultLimit, {
						total: snapshot.resultCount,
					});
				}
				return intl.formatMessage(projectedTerminalMessages.findResult, {
					current: Math.max(1, snapshot.activeIndex + 1),
					total: snapshot.resultCount,
				});
			case 'idle':
				return intl.formatMessage(projectedTerminalMessages.findIdle);
		}
	};
	const statusText = formatStatus();
	const compactStatusText = (() => {
		if (snapshot.notice !== null) return null;
		switch (snapshot.searchState) {
			case 'searching':
				return '…';
			case 'complete': {
				const total = intl.formatNumber(snapshot.resultCount);
				if (snapshot.resultCount === 0) return `${total}/${total}`;
				if (snapshot.resultLimitReached) return `${total}+`;
				const current = Math.min(snapshot.resultCount, Math.max(1, snapshot.activeIndex + 1));
				return `${intl.formatNumber(current)}/${total}`;
			}
			case 'error':
			case 'idle':
			case 'timeout':
				return null;
		}
	})();
	const messageStatusText =
		snapshot.notice !== null ||
		snapshot.searchState === 'error' ||
		snapshot.searchState === 'timeout'
			? statusText
			: null;

	return {
		caseSensitiveLabel: intl.formatMessage(projectedTerminalMessages.matchCase),
		closeLabel: intl.formatMessage(projectedTerminalMessages.closeFind),
		compactStatusText,
		findLabel: intl.formatMessage(projectedTerminalMessages.findLabel),
		inputReference,
		messageStatusText,
		nextLabel: intl.formatMessage(projectedTerminalMessages.nextMatch),
		onCaseSensitiveChange: (pressed: boolean, event: Event) =>
			updateOption((value) => controller.setCaseSensitive(value), pressed, event),
		onClose: () => controller.close(),
		onInputChange: (event: React.ChangeEvent<HTMLInputElement>) =>
			controller.setQuery(event.currentTarget.value),
		onInputKeyDown: (event: React.KeyboardEvent<HTMLInputElement>) => {
			if (event.nativeEvent.isComposing) return;
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
		onOptionControlsKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => {
			if (
				event.nativeEvent.isComposing ||
				event.altKey ||
				event.ctrlKey ||
				event.metaKey ||
				event.shiftKey ||
				(event.key !== 'ArrowLeft' && event.key !== 'ArrowRight')
			) {
				return;
			}
			const options = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('button')];
			const activeElement = event.currentTarget.ownerDocument.activeElement;
			const currentIndex = options.findIndex((option) => option === activeElement);
			if (currentIndex < 0) return;
			const offset = event.key === 'ArrowRight' ? 1 : -1;
			const nextIndex = (currentIndex + offset + options.length) % options.length;
			event.preventDefault();
			event.stopPropagation();
			options[nextIndex]?.focus({ preventScroll: true });
		},
		onPrevious: invokePrevious,
		onRegexChange: (pressed: boolean, event: Event) =>
			updateOption((value) => controller.setRegex(value), pressed, event),
		onSurfaceKeyDown: (event: React.KeyboardEvent<HTMLElement>) => {
			if (event.nativeEvent.isComposing || event.key !== 'Escape') return;
			event.preventDefault();
			event.stopPropagation();
			controller.close();
		},
		onWholeWordChange: (pressed: boolean, event: Event) =>
			updateOption((value) => controller.setWholeWord(value), pressed, event),
		placeholder: intl.formatMessage(projectedTerminalMessages.findPlaceholder),
		previousLabel: intl.formatMessage(projectedTerminalMessages.previousMatch),
		regexLabel: intl.formatMessage(projectedTerminalMessages.useRegex),
		snapshot,
		statusId,
		statusText,
		wholeWordLabel: intl.formatMessage(projectedTerminalMessages.wholeWord),
	};
}
