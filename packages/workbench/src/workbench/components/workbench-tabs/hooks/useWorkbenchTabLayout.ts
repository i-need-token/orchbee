import * as React from 'react';

import type { WorkbenchTab } from '../../../../workbench-tabs';
import {
	createWorkbenchTabWidthLock,
	getWorkbenchTabLockedWidth,
	isWorkbenchTabWidthLockApplicable,
	readWorkbenchPixelToken,
	resolveWorkbenchTabLayoutMode,
} from '../utils/workbench-tab-layout';

const CLOSE_VISIBILITY_WIDTH_TOKEN = '--ob-comp-workbench-tab-close-visibility-width';
const ICON_ONLY_WIDTH_TOKEN = '--ob-comp-workbench-tab-icon-only-width';
const LOCKED_WIDTH_TOKEN = '--ob-comp-workbench-tab-locked-width';
interface WorkbenchTabLayoutProperties {
	readonly 'data-workbench-tab-width-locked': boolean | undefined;
}

interface WorkbenchTabLayout {
	readonly beginPointerClose: React.PointerEventHandler<HTMLButtonElement>;
	readonly getTabLayoutProperties: (tabId: string) => WorkbenchTabLayoutProperties;
	readonly onPointerLeave: React.PointerEventHandler<HTMLDivElement>;
	readonly releaseWidthLock: () => void;
}

export function useWorkbenchTabLayout(
	tabs: readonly WorkbenchTab[],
	resetSequence: number,
	tabListRef: React.RefObject<HTMLDivElement | null>,
): WorkbenchTabLayout {
	const [widthLock, setWidthLock] =
		React.useState<ReturnType<typeof createWorkbenchTabWidthLock>>(undefined);
	const topologyKey = tabs
		.map((tab) => `${tab.id}:${tab.isPinned === true ? 'pinned' : 'open'}:${tab.groupId ?? ''}`)
		.join('|');
	const applicableLock =
		widthLock !== undefined && isWorkbenchTabWidthLockApplicable(widthLock, tabs, resetSequence)
			? widthLock
			: undefined;

	React.useLayoutEffect(() => {
		const tabList = tabListRef.current;
		if (tabList === null) return;
		for (const element of tabList.querySelectorAll<HTMLElement>('[data-workbench-tab-item-id]')) {
			const tabId = element.dataset['workbenchTabItemId'];
			const width =
				tabId === undefined ? undefined : getWorkbenchTabLockedWidth(applicableLock, tabId);
			if (width === undefined) element.style.removeProperty(LOCKED_WIDTH_TOKEN);
			else element.style.setProperty(LOCKED_WIDTH_TOKEN, `${width}px`);
		}
	}, [applicableLock, tabListRef, topologyKey]);

	React.useLayoutEffect(() => {
		const tabList = tabListRef.current;
		const browserWindow = tabList?.ownerDocument.defaultView;
		const ResizeObserverConstructor = browserWindow?.ResizeObserver;
		if (
			tabList === null ||
			tabList === undefined ||
			browserWindow === null ||
			browserWindow === undefined ||
			ResizeObserverConstructor === undefined
		)
			return;
		const closeVisibilityWidth = readWorkbenchPixelToken(tabList, CLOSE_VISIBILITY_WIDTH_TOKEN);
		const iconOnlyWidth = readWorkbenchPixelToken(tabList, ICON_ONLY_WIDTH_TOKEN);
		if (closeVisibilityWidth === undefined || iconOnlyWidth === undefined) return;
		const updateMode = (element: HTMLElement, width: number) => {
			element.dataset['workbenchTabLayout'] = resolveWorkbenchTabLayoutMode(width, {
				closeVisibilityWidth,
				iconOnlyWidth,
			});
		};
		const observer = new ResizeObserverConstructor((entries) => {
			for (const entry of entries) {
				if (entry.target instanceof browserWindow.HTMLElement) {
					updateMode(entry.target, entry.contentRect.width);
				}
			}
		});
		for (const element of tabList.querySelectorAll<HTMLElement>('[data-workbench-tab-item-id]')) {
			updateMode(element, element.getBoundingClientRect().width);
			observer.observe(element);
		}
		return () => observer.disconnect();
	}, [tabListRef, topologyKey]);

	const releaseWidthLock = React.useCallback(() => setWidthLock(undefined), []);
	return {
		beginPointerClose: (event) => {
			if (event.pointerType !== 'mouse' || applicableLock !== undefined) return;
			const tabList = tabListRef.current;
			if (tabList === null) return;
			const measurements = Array.from(
				tabList.querySelectorAll<HTMLElement>('[data-workbench-tab-item-id]'),
				(element) => ({
					id: element.dataset['workbenchTabItemId'] ?? '',
					width: element.getBoundingClientRect().width,
				}),
			).filter(({ id, width }) => id.length > 0 && width > 0);
			setWidthLock(createWorkbenchTabWidthLock(tabs, measurements, resetSequence));
		},
		getTabLayoutProperties: (tabId) => {
			const width = getWorkbenchTabLockedWidth(applicableLock, tabId);
			return {
				'data-workbench-tab-width-locked': width === undefined ? undefined : true,
			};
		},
		onPointerLeave: (event) => {
			if (event.pointerType === 'mouse') releaseWidthLock();
		},
		releaseWidthLock,
	};
}
