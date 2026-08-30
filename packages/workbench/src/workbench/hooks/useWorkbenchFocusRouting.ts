import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import {
	resolveWorkbenchFocusTarget,
	type WorkbenchFocusIntent,
	type WorkbenchFocusRegion,
	type WorkbenchFocusResolution,
} from '../model/resolveWorkbenchFocusTarget';
import type { WorkbenchFocusRouting } from '../types/workbench-focus-routing';
import {
	hasWorkbenchExclusiveFocusSurface,
	isWorkbenchExclusiveFocusTarget,
	resolveWorkbenchFocusRegionFromTarget,
} from '../utils/workbench-focus-dom';

type WorkbenchFocusOwner = WorkbenchFocusRegion | 'exclusive' | null;

export function useWorkbenchFocusRouting({
	isSidebarVisible,
	resolveTerminalTarget,
}: {
	readonly isSidebarVisible: boolean;
	readonly resolveTerminalTarget: () => string | null;
}): WorkbenchFocusRouting {
	const initialOwner: WorkbenchFocusOwner = resolveTerminalTarget() === null ? null : 'terminal';
	const [owner, setOwner] = React.useState<WorkbenchFocusOwner>(initialOwner);
	const [sidebarFocusRequest, setSidebarFocusRequest] = React.useState(0);
	const [tabStripFocusRequest, setTabStripFocusRequest] = React.useState(0);
	const [exclusiveSurfaceMounted, setExclusiveSurfaceMounted] = React.useState(false);
	const terminalFocusSequenceReference = React.useRef(0);
	const [terminalFocusRequest, setTerminalFocusRequest] =
		React.useState<WorkbenchFocusRouting['terminalFocusRequest']>(null);

	const updateOwner = useMemoizedFn((nextOwner: WorkbenchFocusOwner): void => {
		setOwner(nextOwner);
	});
	const availableRegions = useMemoizedFn((): readonly WorkbenchFocusRegion[] => [
		...(resolveTerminalTarget() === null ? [] : (['terminal'] as const)),
		'tab-strip',
		...(isSidebarVisible ? (['sidebar'] as const) : []),
	]);
	const requestTerminalFocus = useMemoizedFn((): boolean => {
		if (typeof document !== 'undefined') {
			if (hasWorkbenchExclusiveFocusSurface(document)) return false;
			setExclusiveSurfaceMounted(false);
		}
		const presentationId = resolveTerminalTarget();
		if (presentationId === null) return false;
		terminalFocusSequenceReference.current += 1;
		updateOwner('terminal');
		setTerminalFocusRequest({
			presentationId,
			sequence: terminalFocusSequenceReference.current,
		});
		return true;
	});
	const getResolution = useMemoizedFn((intent: WorkbenchFocusIntent): WorkbenchFocusResolution => {
		const activeElement = typeof document === 'undefined' ? null : document.activeElement;
		const focusedRegion = resolveWorkbenchFocusRegionFromTarget(activeElement);
		const exclusiveSurfaceActive =
			typeof document === 'undefined'
				? exclusiveSurfaceMounted
				: hasWorkbenchExclusiveFocusSurface(document) ||
					isWorkbenchExclusiveFocusTarget(activeElement);
		return resolveWorkbenchFocusTarget({
			availableRegions: availableRegions(),
			currentRegion: focusedRegion,
			exclusiveSurfaceActive,
			intent,
		});
	});
	const focus = useMemoizedFn((intent: WorkbenchFocusIntent): WorkbenchFocusResolution => {
		const resolution = getResolution(intent);
		if (resolution.kind !== 'target') return resolution;

		updateOwner(resolution.region);
		switch (resolution.region) {
			case 'sidebar':
				setSidebarFocusRequest((current) => current + 1);
				break;
			case 'tab-strip':
				setTabStripFocusRequest((current) => current + 1);
				break;
			case 'terminal':
				requestTerminalFocus();
				break;
		}
		return resolution;
	});
	const onFocusCapture = useMemoizedFn((event: React.FocusEvent<HTMLDivElement>): void => {
		if (isWorkbenchExclusiveFocusTarget(event.target)) {
			updateOwner('exclusive');
			return;
		}
		if (hasWorkbenchExclusiveFocusSurface(event.currentTarget.ownerDocument)) {
			updateOwner('exclusive');
			return;
		}
		const region = resolveWorkbenchFocusRegionFromTarget(event.target);
		if (region !== null && event.currentTarget.contains(event.target)) updateOwner(region);
	});
	const onBlurCapture = useMemoizedFn((event: React.FocusEvent<HTMLDivElement>): void => {
		if (isWorkbenchExclusiveFocusTarget(event.relatedTarget)) {
			updateOwner('exclusive');
			return;
		}
		if (event.relatedTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)) {
			return;
		}
		if (hasWorkbenchExclusiveFocusSurface(event.currentTarget.ownerDocument)) {
			updateOwner('exclusive');
			return;
		}
		const nextRegion = resolveWorkbenchFocusRegionFromTarget(event.relatedTarget);
		updateOwner(nextRegion);
	});
	React.useEffect(() => {
		if (typeof document === 'undefined' || typeof MutationObserver === 'undefined') {
			return undefined;
		}
		const reconcileExclusiveSurface = (): void => {
			setExclusiveSurfaceMounted(hasWorkbenchExclusiveFocusSurface(document));
		};
		reconcileExclusiveSurface();
		const observer = new MutationObserver(reconcileExclusiveSurface);
		observer.observe(document.body, { childList: true, subtree: true });
		return () => observer.disconnect();
	}, []);
	React.useEffect(() => {
		setOwner((currentOwner) => {
			if (exclusiveSurfaceMounted) return 'exclusive';
			if (currentOwner !== 'exclusive') return currentOwner;
			return typeof document === 'undefined'
				? null
				: resolveWorkbenchFocusRegionFromTarget(document.activeElement);
		});
	}, [exclusiveSurfaceMounted]);

	return {
		focus,
		getResolution,
		onBlurCapture,
		onFocusCapture,
		sidebarFocusRequest,
		tabStripFocusRequest,
		terminalAutoFocusAllowed: !exclusiveSurfaceMounted && (owner === 'terminal' || owner === null),
		terminalFocusRequest,
		requestTerminalFocus,
	};
}
