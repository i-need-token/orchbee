import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileOverlayMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { useMemo, useRef, type KeyboardEvent, type ReactNode } from 'react';

import { fileTabLabels } from '../../../../../../../model';
import {
	useFileOverlayHostCommands,
	useFileOverlayHostStore,
} from '../../../../contexts/file-overlay-host-store';
import { fileTabDomId } from '../../utils/fileTabDomId';
import { FileTab } from './components/file-tab/FileTab';
import { useFileTabFocusHandoff } from './hooks/useFileTabFocusHandoff';
import { useFileTabReveal } from './hooks/useFileTabReveal';
import { useFileTabRowWheel } from './hooks/useFileTabRowWheel';
import styles from './FileTabRow.module.css';

interface FileTabRowProps {
	/** The prefix of the tabs' element ids, by which the content panel names the active tab. */
	readonly idPrefix: string;
	/** The content panel's element id, which the active tab controls. */
	readonly panelId: string;
}

/** The tab a key moves the focus to from the tab at `index`; VS Code's tabs do not wrap. */
function tabIndexForKey(key: string, index: number, count: number): number | null {
	switch (key) {
		case 'ArrowLeft':
		case 'ArrowUp':
			return Math.max(index - 1, 0);
		case 'ArrowRight':
		case 'ArrowDown':
			return Math.min(index + 1, count - 1);
		case 'Home':
			return 0;
		case 'End':
			return count - 1;
		default:
			return null;
	}
}

/**
 * The overlay's file tabs, one row as VS Code's single editor group shows them: sticky tabs first,
 * each tab as wide as its label, the row scrolling sideways when they do not fit. Its scrollbar is
 * hidden, so the row stays the height of the Tab bar; the wheel, a sideways swipe and the keys still
 * scroll it, and the active tab always scrolls into view. The keys move along the row and show each
 * tab they reach, and Enter or Space shows the focused one.
 */
export function FileTabRow({ idPrefix, panelId }: FileTabRowProps): ReactNode {
	const intl = useOrchbeeIntl();
	const tabs = useFileOverlayHostStore((state) => state.tabs);
	const commands = useFileOverlayHostCommands();
	const rowReference = useRef<HTMLDivElement>(null);
	const listReference = useRef<HTMLDivElement>(null);
	const labels = useMemo(
		() => fileTabLabels(tabs.tabs.map((tab) => tab.input.resource.path)),
		[tabs.tabs],
	);
	useFileTabRowWheel(rowReference);
	useFileTabReveal(rowReference, listReference, tabs.activeId, tabs.tabs.length);
	const handOffFocus = useFileTabFocusHandoff(rowReference, tabs.activeId);
	const onKeyDown = useMemoizedFn((event: KeyboardEvent<HTMLDivElement>) => {
		const list = listReference.current;
		if (list === null || event.nativeEvent.isComposing) return;
		if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
		const elements = [...list.querySelectorAll<HTMLElement>('[role="tab"]')];
		const index = elements.findIndex((element) => element === event.target);
		const tab = tabs.tabs[index];
		if (tab === undefined) return;
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			if (!event.repeat) commands.selectTab(tab.id, 'keyboard');
			return;
		}
		const nextIndex = tabIndexForKey(event.key, index, elements.length);
		if (nextIndex === null) return;
		event.preventDefault();
		const next = tabs.tabs[nextIndex];
		if (nextIndex === index || next === undefined) return;
		commands.selectTab(next.id, 'keyboard');
		elements[nextIndex]?.focus({ preventScroll: true });
	});
	return (
		<div className={styles['root']} data-slot='file-tab-row' ref={rowReference}>
			<div
				aria-label={intl.formatMessage(fileOverlayMessages.tabsLabel)}
				className={styles['list']}
				ref={listReference}
				role='tablist'
				onKeyDown={onKeyDown}
			>
				{tabs.tabs.map((tab, index) => (
					<FileTab
						active={tab.id === tabs.activeId}
						description={labels[index]?.description ?? ''}
						domId={fileTabDomId(idPrefix, index)}
						key={tab.id}
						name={labels[index]?.name ?? tab.input.resource.path}
						panelId={panelId}
						path={tab.input.resource.path}
						preview={tab.id === tabs.previewId}
						sticky={index < tabs.stickyCount}
						tabId={tab.id}
						unsaved={tab.unsaved}
						onLeaveWithFocus={handOffFocus}
					/>
				))}
			</div>
		</div>
	);
}
