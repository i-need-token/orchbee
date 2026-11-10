import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import {
	resolveWorkbenchFocusTarget,
	type WorkbenchFocusIntent,
	type WorkbenchFocusRegion,
	type WorkbenchFocusResolution,
} from '../model/resolveWorkbenchFocusTarget';
import type {
	WorkbenchFocusInvocation,
	WorkbenchFocusRouting,
	WorkbenchTerminalHandover,
} from '../types/workbench-focus-routing';
import {
	hasWorkbenchCommandSurface,
	hasWorkbenchExclusiveFocusSurface,
	isWorkbenchExclusiveFocusTarget,
	resolveWorkbenchFocusRegionFromTarget,
} from '../utils/workbench-focus-dom';

type WorkbenchFocusOwner = WorkbenchFocusRegion | 'exclusive' | null;

export function useWorkbenchFocusRouting({
	isSidebarVisible,
	isTabStripVisible = true,
	resolveTerminalTarget,
}: {
	readonly isSidebarVisible: boolean;
	/** False while the mobile layout's Tab list, which holds its strip, is closed. */
	readonly isTabStripVisible?: boolean | undefined;
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
	const interactionSequenceReference = React.useRef(0);
	/** The region that takes focus once a closing menu or dialog has given focus back. */
	const pendingFocusRegionReference = React.useRef<WorkbenchFocusRegion | null>(null);
	/** The region of the last focus outside any menu or dialog, where a closing one returns it. */
	const focusReturnRegionReference = React.useRef<WorkbenchFocusRegion | null>(null);

	const updateOwner = useMemoizedFn((nextOwner: WorkbenchFocusOwner): void => {
		setOwner(nextOwner);
	});
	const availableRegions = useMemoizedFn((): readonly WorkbenchFocusRegion[] => [
		...(resolveTerminalTarget() === null ? [] : (['terminal'] as const)),
		...(isTabStripVisible ? (['tab-strip'] as const) : []),
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
	const beginTerminalHandover = useMemoizedFn((): WorkbenchTerminalHandover => {
		const claimedInteraction = interactionSequenceReference.current;
		const held = (): boolean => interactionSequenceReference.current === claimedInteraction;
		return {
			complete: () => {
				if (!held()) return;
				// A closing menu or dialog still gives focus back, so the terminal takes it after that.
				if (typeof document !== 'undefined' && hasWorkbenchExclusiveFocusSurface(document)) {
					pendingFocusRegionReference.current = 'terminal';
					return;
				}
				requestTerminalFocus();
			},
			held,
		};
	});
	/** Whether a row of the open command surface asks, so the move waits for it to close. */
	const asksFromOpenCommandSurface = (invocation: WorkbenchFocusInvocation | undefined) =>
		invocation?.fromCommandSurface === true &&
		typeof document !== 'undefined' &&
		hasWorkbenchCommandSurface(document);
	const getResolution = useMemoizedFn(
		(
			intent: WorkbenchFocusIntent,
			invocation?: WorkbenchFocusInvocation,
		): WorkbenchFocusResolution => {
			const activeElement = typeof document === 'undefined' ? null : document.activeElement;
			const fromCommandSurface = asksFromOpenCommandSurface(invocation);
			const exclusiveSurfaceActive =
				typeof document === 'undefined'
					? exclusiveSurfaceMounted
					: hasWorkbenchExclusiveFocusSurface(document, fromCommandSurface) ||
						isWorkbenchExclusiveFocusTarget(activeElement, fromCommandSurface);
			return resolveWorkbenchFocusTarget({
				availableRegions: availableRegions(),
				currentRegion: fromCommandSurface
					? focusReturnRegionReference.current
					: resolveWorkbenchFocusRegionFromTarget(activeElement),
				exclusiveSurfaceActive,
				intent,
			});
		},
	);
	const focusRegion = useMemoizedFn((region: WorkbenchFocusRegion): void => {
		if (!availableRegions().includes(region)) return;
		updateOwner(region);
		switch (region) {
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
	});
	const focus = useMemoizedFn(
		(
			intent: WorkbenchFocusIntent,
			invocation?: WorkbenchFocusInvocation,
		): WorkbenchFocusResolution => {
			const resolution = getResolution(intent, invocation);
			if (resolution.kind !== 'target') return resolution;
			// The closing command surface still gives focus back, so the target takes it after that.
			if (asksFromOpenCommandSurface(invocation)) {
				pendingFocusRegionReference.current = resolution.region;
				return resolution;
			}
			focusRegion(resolution.region);
			return resolution;
		},
	);
	const onFocusCapture = useMemoizedFn((event: React.FocusEvent<HTMLDivElement>): void => {
		if (isWorkbenchExclusiveFocusTarget(event.target)) {
			updateOwner('exclusive');
			return;
		}
		if (hasWorkbenchExclusiveFocusSurface(event.currentTarget.ownerDocument)) {
			updateOwner('exclusive');
			return;
		}
		if (!event.currentTarget.contains(event.target)) return;
		const region = resolveWorkbenchFocusRegionFromTarget(event.target);
		focusReturnRegionReference.current = region;
		if (region !== null) updateOwner(region);
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
	React.useEffect(() => {
		const region = pendingFocusRegionReference.current;
		if (exclusiveSurfaceMounted || region === null) return;
		pendingFocusRegionReference.current = null;
		focusRegion(region);
	}, [exclusiveSurfaceMounted, focusRegion]);
	React.useEffect(() => {
		const countInteraction = (): void => {
			interactionSequenceReference.current += 1;
			pendingFocusRegionReference.current = null;
		};
		// Ahead of every document-level shortcut, so a press never revokes the request it starts.
		window.addEventListener('keydown', countInteraction, true);
		window.addEventListener('pointerdown', countInteraction, true);
		return () => {
			window.removeEventListener('keydown', countInteraction, true);
			window.removeEventListener('pointerdown', countInteraction, true);
		};
	}, []);

	return {
		beginTerminalHandover,
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
