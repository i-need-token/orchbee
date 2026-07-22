import type { WorkbenchTab } from '../../../../workbench-tabs';

export type WorkbenchTabLayoutMode = 'compact' | 'icon-only' | 'standard';

interface WorkbenchTabLayoutThresholds {
	readonly closeVisibilityWidth: number;
	readonly iconOnlyWidth: number;
}

interface WorkbenchTabWidthMeasurement {
	readonly id: string;
	readonly width: number;
}

interface WorkbenchTabWidthLockEntry {
	readonly groupId: string | undefined;
	readonly id: string;
	readonly isPinned: boolean;
	readonly width: number | undefined;
}

export interface WorkbenchTabWidthLock {
	readonly resetSequence: number;
	readonly tabs: readonly WorkbenchTabWidthLockEntry[];
}

export function resolveWorkbenchTabLayoutMode(
	width: number,
	thresholds: WorkbenchTabLayoutThresholds,
): WorkbenchTabLayoutMode {
	if (width <= thresholds.iconOnlyWidth) return 'icon-only';
	if (width <= thresholds.closeVisibilityWidth) return 'compact';
	return 'standard';
}

export function readWorkbenchPixelToken(
	element: HTMLElement,
	tokenName: string,
): number | undefined {
	const browserWindow = element.ownerDocument.defaultView;
	const value = browserWindow?.getComputedStyle(element).getPropertyValue(tokenName).trim();
	if (value === undefined || !value.endsWith('px')) return undefined;
	const pixels = Number.parseFloat(value);
	return Number.isFinite(pixels) ? pixels : undefined;
}

export function createWorkbenchTabWidthLock(
	tabs: readonly WorkbenchTab[],
	measurements: readonly WorkbenchTabWidthMeasurement[],
	resetSequence: number,
): WorkbenchTabWidthLock | undefined {
	const widths = new Map(measurements.map(({ id, width }) => [id, width]));
	if (widths.size === 0) return undefined;
	return {
		resetSequence,
		tabs: tabs.map((tab) => ({
			groupId: tab.groupId,
			id: tab.id,
			isPinned: tab.isPinned === true,
			width: widths.get(tab.id),
		})),
	};
}

export function isWorkbenchTabWidthLockApplicable(
	lock: WorkbenchTabWidthLock,
	tabs: readonly WorkbenchTab[],
	resetSequence: number,
): boolean {
	if (lock.resetSequence !== resetSequence) return false;
	let lockedIndex = 0;
	for (const tab of tabs) {
		while (lockedIndex < lock.tabs.length && lock.tabs[lockedIndex]?.id !== tab.id) {
			lockedIndex += 1;
		}
		const lockedTab = lock.tabs[lockedIndex];
		if (
			lockedTab === undefined ||
			lockedTab.groupId !== tab.groupId ||
			lockedTab.isPinned !== (tab.isPinned === true)
		) {
			return false;
		}
		lockedIndex += 1;
	}
	return true;
}

export function getWorkbenchTabLockedWidth(
	lock: WorkbenchTabWidthLock | undefined,
	tabId: string,
): number | undefined {
	return lock?.tabs.find(({ id }) => id === tabId)?.width;
}
