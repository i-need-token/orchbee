import * as React from 'react';

import type {
	WorkbenchTab,
	WorkbenchTabGroup,
	WorkbenchTabsState,
} from '../../../../workbench-tabs';
import type {
	WorkbenchTabDragCommand,
	WorkbenchTabDragEvent,
	WorkbenchTabDragState,
	WorkbenchTabStripItemGeometry,
} from '../types/workbench-tab-drag';
import { canArmWorkbenchTabDrag } from '../utils/canArmWorkbenchTabDrag';
import { resolveWorkbenchTabDragReconciliation } from '../utils/resolveWorkbenchTabDragReconciliation';
import { transitionWorkbenchTabDrag } from '../utils/transitionWorkbenchTabDrag';
import {
	getWorkbenchTabsTopologyKey,
	hasWorkbenchTabDragActivePointer,
} from '../utils/workbench-tab-drag-state';

interface WorkbenchTabDragProperties {
	readonly 'data-dragging': boolean | undefined;
	readonly onClickCapture: React.MouseEventHandler<HTMLDivElement>;
	readonly onLostPointerCapture: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerCancel: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerDown: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerMove: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerUp: React.PointerEventHandler<HTMLDivElement>;
	readonly reorderable: boolean;
}

type WorkbenchTabGroupDragProperties = WorkbenchTabDragProperties;

interface WorkbenchTabDrag {
	readonly dragOverlayRef: React.RefObject<HTMLDivElement | null>;
	readonly dragPlaceholderRef: React.RefObject<HTMLDivElement | null>;
	readonly draggedGroupId: string | undefined;
	readonly draggedTabId: string | undefined;
	readonly getTabGroupDragProperties: (groupId: string) => WorkbenchTabGroupDragProperties;
	readonly getTabDragProperties: (tabId: string) => WorkbenchTabDragProperties;
	readonly hoverSuppressed: boolean;
	readonly isReordering: boolean;
	readonly onKeyDown: React.KeyboardEventHandler<HTMLDivElement>;
	readonly onLostPointerCapture: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerCancel: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerMove: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerUp: React.PointerEventHandler<HTMLDivElement>;
	readonly tabGroups: readonly WorkbenchTabGroup[];
	readonly tabListRef: React.RefObject<HTMLDivElement | null>;
	readonly tabs: readonly WorkbenchTab[];
}

const IDLE_DRAG_STATE: WorkbenchTabDragState = { kind: 'idle' };
const GROUP_LINE_OVERLAP = 1;

export function useWorkbenchTabDrag(
	authoritativeState: WorkbenchTabsState,
	onTabMove:
		| ((tabId: string, targetIndex: number, groupId: string | undefined) => void)
		| undefined,
	onTabGroupMove: ((groupId: string, targetIndex: number) => void) | undefined,
	onTabDragStart: ((tabId: string) => void) | undefined = undefined,
	onTabGroupDragStart: ((groupId: string) => void) | undefined = undefined,
): WorkbenchTabDrag {
	const [dragState, setDragState] = React.useState<WorkbenchTabDragState>(IDLE_DRAG_STATE);
	const [hoverSuppressed, setHoverSuppressed] = React.useState(false);
	const dragOverlayRef = React.useRef<HTMLDivElement>(null);
	const dragPlaceholderRef = React.useRef<HTMLDivElement>(null);
	const tabListRef = React.useRef<HTMLDivElement>(null);
	const dragStateRef = React.useRef<WorkbenchTabDragState>(IDLE_DRAG_STATE);
	const draggedGroupElementRef = React.useRef<HTMLDivElement | undefined>(undefined);
	const draggedTabElementRef = React.useRef<HTMLDivElement | undefined>(undefined);
	const dragOverlaySurfaceRef = React.useRef<HTMLDivElement | undefined>(undefined);
	const previousPositionsRef = React.useRef<ReadonlyMap<string, number> | undefined>(undefined);
	const clickReleaseFrameRef = React.useRef<number | undefined>(undefined);
	const suppressClickRef = React.useRef(false);
	const applyEventRef = React.useRef<(event: WorkbenchTabDragEvent) => void>(() => undefined);
	const clearDraggedStylesRef = React.useRef<() => void>(() => undefined);
	const topologyKey = getWorkbenchTabsTopologyKey(authoritativeState);
	const lastTopologyKeyRef = React.useRef(topologyKey);
	const displayState =
		dragState.kind === 'dragging-tab' ||
		dragState.kind === 'dragging-group' ||
		dragState.kind === 'settling'
			? dragState.previewState
			: authoritativeState;
	const draggedTabId =
		dragState.kind === 'armed-tab' || dragState.kind === 'dragging-tab'
			? dragState.draggedTabId
			: undefined;
	const draggedGroupId =
		dragState.kind === 'armed-group' || dragState.kind === 'dragging-group'
			? dragState.draggedGroupId
			: undefined;
	const isReordering = dragState.kind === 'dragging-tab' || dragState.kind === 'dragging-group';

	const getDraggedGroupElements = (groupId: string): readonly HTMLDivElement[] => {
		const tabList = tabListRef.current;
		if (tabList === null) return [];
		const elements: HTMLDivElement[] = [];
		for (const itemElement of tabList.querySelectorAll<HTMLDivElement>(
			'[data-workbench-tab-motion-key]',
		)) {
			if (itemElement.dataset['workbenchTabMotionKey'] === `group:${groupId}`) {
				elements.push(itemElement);
				break;
			}
		}
		for (const memberElement of tabList.querySelectorAll<HTMLDivElement>(
			'[data-workbench-tab-member-group-id]',
		)) {
			if (memberElement.dataset['workbenchTabMemberGroupId'] === groupId) {
				elements.push(memberElement);
			}
		}
		return elements;
	};

	const capturePositions = () => {
		const tabList = tabListRef.current;
		if (tabList === null) return;
		const positions = new Map<string, number>();
		for (const element of tabList.querySelectorAll<HTMLElement>(
			'[data-workbench-tab-motion-key]',
		)) {
			const key = element.dataset['workbenchTabMotionKey'];
			if (key === undefined) continue;
			positions.set(key, element.getBoundingClientRect().left);
			for (const animation of element.getAnimations()) animation.cancel();
		}
		previousPositionsRef.current = positions;
	};

	const measureIdealLayout = (): readonly WorkbenchTabStripItemGeometry[] => {
		const tabList = tabListRef.current;
		if (tabList === null) return [];
		const geometry: WorkbenchTabStripItemGeometry[] = [];
		for (const element of tabList.querySelectorAll<HTMLElement>(
			'[data-workbench-tab-motion-key]',
		)) {
			const key = element.dataset['workbenchTabMotionKey'];
			if (key === undefined) continue;
			const separatorIndex = key.indexOf(':');
			if (separatorIndex === -1) continue;
			const kind = key.slice(0, separatorIndex);
			const id = key.slice(separatorIndex + 1);
			if (id.length === 0 || (kind !== 'group' && kind !== 'tab')) continue;
			geometry.push({ id, kind, left: element.offsetLeft, width: element.offsetWidth });
		}
		return geometry;
	};

	const beginDragOverlay = () => {
		const overlay = dragOverlayRef.current;
		const tabList = tabListRef.current;
		const currentState = dragStateRef.current;
		if (
			overlay === null ||
			tabList === null ||
			(currentState.kind !== 'dragging-tab' && currentState.kind !== 'dragging-group')
		)
			return;
		let sourceElements: readonly HTMLDivElement[] = [];
		if (currentState.kind === 'dragging-group') {
			sourceElements = getDraggedGroupElements(currentState.draggedGroupId);
		} else if (draggedTabElementRef.current !== undefined) {
			sourceElements = [draggedTabElementRef.current];
		}
		if (sourceElements.length === 0) return;
		overlay.replaceChildren();
		const surface = overlay.ownerDocument.createElement('div');
		surface.dataset['workbenchTabDragSurface'] = '';
		surface.setAttribute('aria-hidden', 'true');
		const renderedSourceLeft = Math.min(...sourceElements.map((element) => element.offsetLeft));
		const sourceRight = Math.max(
			...sourceElements.map((element) => element.offsetLeft + element.offsetWidth),
		);
		surface.style.left = `${currentState.sourceLeft}px`;
		surface.style.width = `${sourceRight - renderedSourceLeft}px`;

		for (const sourceElement of sourceElements) {
			const clone = sourceElement.cloneNode(true);
			if (!(clone instanceof HTMLElement)) continue;
			clone.dataset['workbenchTabDragClone'] = '';
			if (currentState.kind === 'dragging-tab') clone.dataset['dragging'] = 'true';
			else if (clone.dataset['workbenchTabMemberGroupId'] === currentState.draggedGroupId) {
				clone.dataset['workbenchTabGroupDragging'] = 'true';
			} else {
				clone.dataset['groupDragging'] = 'true';
			}
			clone.style.left = `${sourceElement.offsetLeft - renderedSourceLeft}px`;
			clone.style.top = `${sourceElement.offsetTop}px`;
			clone.style.width = `${sourceElement.offsetWidth}px`;
			clone.style.height = `${sourceElement.offsetHeight}px`;
			surface.append(clone);
		}

		overlay.append(surface);
		dragOverlaySurfaceRef.current = surface;
	};

	const synchronizeDraggedTabOverlayGroupVisual = () => {
		const currentState = dragStateRef.current;
		const surface = dragOverlaySurfaceRef.current;
		if (currentState.kind !== 'dragging-tab' || surface === undefined) return;
		const clone = surface.querySelector<HTMLElement>('[data-workbench-tab-drag-clone]');
		if (clone === null) return;
		const previewTab = currentState.previewState.tabs.find(
			(tab) => tab.id === currentState.draggedTabId,
		);
		const previewGroup = currentState.previewState.tabGroups.find(
			(group) => group.id === previewTab?.groupId,
		);
		if (previewTab?.groupId === undefined || previewGroup === undefined) {
			delete clone.dataset['workbenchTabMemberGroupId'];
			delete clone.dataset['workbenchTabGroupColor'];
			return;
		}
		clone.dataset['workbenchTabMemberGroupId'] = previewTab.groupId;
		clone.dataset['workbenchTabGroupColor'] = previewGroup.color;
	};

	const updateDraggedItemPosition = () => {
		const currentState = dragStateRef.current;
		if (currentState.kind !== 'dragging-group' && currentState.kind !== 'dragging-tab') return;
		const surface = dragOverlaySurfaceRef.current;
		if (surface === undefined) return;
		const nextOffset = currentState.pointerX - currentState.grabOffset - currentState.sourceLeft;
		surface.style.transform = `translate3d(${nextOffset}px, 0, 0)`;
		if (currentState.kind !== 'dragging-tab') return;
		const draggedElement = draggedTabElementRef.current;
		const tabList = tabListRef.current;
		if (draggedElement === undefined || tabList === null) return;
		const placeholder = dragPlaceholderRef.current;
		if (placeholder === null) return;
		placeholder.style.width = `${draggedElement.offsetWidth + GROUP_LINE_OVERLAP * 2}px`;
		placeholder.style.transform = `translate3d(${draggedElement.offsetLeft - GROUP_LINE_OVERLAP}px, 0, 0)`;
	};

	const executeCommands = (commands: readonly WorkbenchTabDragCommand[]) => {
		for (const command of commands) {
			switch (command.kind) {
				case 'activate-tab':
					onTabDragStart?.(command.tabId);
					break;
				case 'activate-tab-group':
					onTabGroupDragStart?.(command.groupId);
					break;
				case 'commit-tab-move':
					onTabMove?.(command.tabId, command.targetIndex, command.groupId);
					break;
				case 'commit-tab-group-move':
					onTabGroupMove?.(command.groupId, command.targetIndex);
					break;
			}
		}
	};

	const clearDraggedStyles = () => {
		dragOverlaySurfaceRef.current = undefined;
		dragOverlayRef.current?.replaceChildren();
		const placeholder = dragPlaceholderRef.current;
		if (placeholder !== null) {
			placeholder.style.transform = '';
			placeholder.style.width = '';
		}
		draggedGroupElementRef.current = undefined;
		draggedTabElementRef.current = undefined;
	};
	clearDraggedStylesRef.current = clearDraggedStyles;

	const applyEvent = (event: WorkbenchTabDragEvent) => {
		const previousState = dragStateRef.current;
		const transition = transitionWorkbenchTabDrag(previousState, event);
		if (transition.state === previousState && transition.commands.length === 0) return transition;
		const previousPreview =
			previousState.kind === 'dragging-tab' ||
			previousState.kind === 'dragging-group' ||
			previousState.kind === 'settling'
				? previousState.previewState
				: authoritativeState;
		const nextPreview =
			transition.state.kind === 'dragging-tab' ||
			transition.state.kind === 'dragging-group' ||
			transition.state.kind === 'settling'
				? transition.state.previewState
				: authoritativeState;
		if (previousPreview.tabs !== nextPreview.tabs) capturePositions();
		const wasDragging =
			previousState.kind === 'dragging-tab' || previousState.kind === 'dragging-group';
		const isDragging =
			transition.state.kind === 'dragging-tab' || transition.state.kind === 'dragging-group';
		dragStateRef.current = transition.state;
		setDragState(transition.state);
		if (wasDragging && !isDragging) {
			setHoverSuppressed(true);
			clearDraggedStyles();
			const browserWindow = tabListRef.current?.ownerDocument.defaultView;
			const previousFrame = clickReleaseFrameRef.current;
			if (previousFrame !== undefined) browserWindow?.cancelAnimationFrame(previousFrame);
			if (browserWindow !== undefined && browserWindow !== null) {
				clickReleaseFrameRef.current = browserWindow.requestAnimationFrame(() => {
					clickReleaseFrameRef.current = undefined;
					suppressClickRef.current = false;
				});
			} else {
				suppressClickRef.current = false;
			}
		}
		executeCommands(transition.commands);
		return transition;
	};
	applyEventRef.current = applyEvent;

	React.useLayoutEffect(() => {
		const tabList = tabListRef.current;
		if (tabList === null) return;
		const previousPositions = previousPositionsRef.current;
		previousPositionsRef.current = undefined;
		if (previousPositions !== undefined) {
			const browserWindow = tabList.ownerDocument.defaultView;
			const motionStyles = browserWindow?.getComputedStyle(tabList);
			const durationValue =
				motionStyles?.getPropertyValue('--ob-sys-motion-duration-fast').trim() ?? '0ms';
			const easing =
				motionStyles?.getPropertyValue('--ob-sys-motion-easing-standard').trim() ?? 'linear';
			const parsedDuration = Number.parseFloat(durationValue);
			let duration = 0;
			if (Number.isFinite(parsedDuration) && parsedDuration > 0) {
				duration = durationValue.endsWith('ms') ? parsedDuration : parsedDuration * 1000;
			}
			const currentState = dragStateRef.current;
			const draggedGroupTabKeys = new Set(
				currentState.kind === 'dragging-group'
					? displayState.tabs
							.filter((tab) => tab.groupId === currentState.draggedGroupId)
							.map((tab) => `tab:${tab.id}`)
					: [],
			);
			for (const element of tabList.querySelectorAll<HTMLElement>(
				'[data-workbench-tab-motion-key]',
			)) {
				const key = element.dataset['workbenchTabMotionKey'];
				const previousLeft = key === undefined ? undefined : previousPositions.get(key);
				const isDragged =
					key === `tab:${draggedTabId ?? ''}` ||
					key === `group:${draggedGroupId ?? ''}` ||
					draggedGroupTabKeys.has(key ?? '');
				if (
					previousLeft === undefined ||
					isDragged ||
					duration === 0 ||
					typeof element.animate !== 'function'
				) {
					continue;
				}
				const offset = previousLeft - element.getBoundingClientRect().left;
				if (Math.abs(offset) < 0.5) continue;
				element.animate(
					[{ transform: `translate3d(${offset}px, 0, 0)` }, { transform: 'translate3d(0, 0, 0)' }],
					{ duration, easing },
				);
			}
		}
		if (isReordering && dragOverlaySurfaceRef.current === undefined) beginDragOverlay();
		synchronizeDraggedTabOverlayGroupVisual();
		updateDraggedItemPosition();
	}, [displayState.tabs, draggedGroupId, draggedTabId, isReordering]);

	React.useEffect(() => {
		const currentState = dragStateRef.current;
		const reconciliationEvent = resolveWorkbenchTabDragReconciliation(
			currentState,
			lastTopologyKeyRef.current,
			topologyKey,
		);
		if (reconciliationEvent !== undefined) applyEventRef.current(reconciliationEvent);
		lastTopologyKeyRef.current = topologyKey;
	}, [topologyKey]);

	React.useEffect(() => {
		const browserWindow = tabListRef.current?.ownerDocument.defaultView;
		return () => {
			const clickReleaseFrame = clickReleaseFrameRef.current;
			if (clickReleaseFrame !== undefined) browserWindow?.cancelAnimationFrame(clickReleaseFrame);
			clearDraggedStylesRef.current();
		};
	}, []);

	const cancelPointer = (pointerId: number) => {
		applyEvent({ kind: 'cancel', pointerId });
		const tabList = tabListRef.current;
		if (tabList?.hasPointerCapture(pointerId) === true) tabList.releasePointerCapture(pointerId);
	};

	React.useEffect(() => {
		if (!isReordering) return;
		const ownerDocument = tabListRef.current?.ownerDocument;
		if (ownerDocument === undefined) return;
		const handleKeyDown = (event: KeyboardEvent) => {
			if (event.key !== 'Escape') return;
			const currentState = dragStateRef.current;
			if (!hasWorkbenchTabDragActivePointer(currentState)) return;
			event.preventDefault();
			event.stopPropagation();
			applyEventRef.current({
				kind: 'cancel',
				pointerId: currentState.pointerId,
			});
			const tabList = tabListRef.current;
			if (tabList?.hasPointerCapture(currentState.pointerId) === true) {
				tabList.releasePointerCapture(currentState.pointerId);
			}
		};
		ownerDocument.addEventListener('keydown', handleKeyDown, true);
		return () => ownerDocument.removeEventListener('keydown', handleKeyDown, true);
	}, [isReordering]);

	const movePointer = (
		pointerId: number,
		pointerX: number,
		pointerY: number,
		primaryButtonPressed: boolean,
	): boolean => {
		const previousState = dragStateRef.current;
		const transition = applyEvent({
			kind: 'move',
			pointerId,
			primaryButtonPressed,
			pointerX,
			pointerY,
		});
		const crossedThreshold =
			(previousState.kind === 'armed-group' && transition.state.kind === 'dragging-group') ||
			(previousState.kind === 'armed-tab' && transition.state.kind === 'dragging-tab');
		const tabList = tabListRef.current;
		if (crossedThreshold && tabList !== null && !tabList.hasPointerCapture(pointerId)) {
			tabList.setPointerCapture(pointerId);
		}
		const isDragging =
			transition.state.kind === 'dragging-group' || transition.state.kind === 'dragging-tab';
		if (isDragging) updateDraggedItemPosition();
		return isDragging;
	};

	const releasePointer = (pointerId: number): boolean => {
		const wasDragging =
			dragStateRef.current.kind === 'dragging-group' ||
			dragStateRef.current.kind === 'dragging-tab';
		if (wasDragging) suppressClickRef.current = true;
		applyEvent({ kind: 'release', pointerId });
		const tabList = tabListRef.current;
		if (tabList?.hasPointerCapture(pointerId) === true) tabList.releasePointerCapture(pointerId);
		return wasDragging;
	};

	const createSharedProperties = (
		isDragged: boolean,
		isReorderable: boolean,
	): Pick<
		WorkbenchTabDragProperties,
		'data-dragging' | 'onClickCapture' | 'onLostPointerCapture' | 'reorderable'
	> => ({
		'data-dragging': isDragged ? true : undefined,
		onClickCapture: (event) => {
			if (!suppressClickRef.current) return;
			event.preventDefault();
			event.stopPropagation();
			suppressClickRef.current = false;
		},
		onLostPointerCapture: (event) => {
			if (hasWorkbenchTabDragActivePointer(dragStateRef.current)) cancelPointer(event.pointerId);
		},
		reorderable: isReorderable,
	});

	const getTabGroupDragProperties = (groupId: string): WorkbenchTabGroupDragProperties => ({
		...createSharedProperties(draggedGroupId === groupId, onTabGroupMove !== undefined),
		onPointerCancel: (event) => cancelPointer(event.pointerId),
		onPointerDown: (event) => {
			if (onTabGroupMove === undefined || !canArmWorkbenchTabDrag(event)) return;
			const tabList = tabListRef.current;
			if (tabList === null) return;
			const tabListBounds = tabList.getBoundingClientRect();
			draggedGroupElementRef.current = event.currentTarget;
			const groupElements = getDraggedGroupElements(groupId);
			const groupRight = Math.max(
				event.currentTarget.offsetLeft + event.currentTarget.offsetWidth,
				...groupElements.map((element) => element.offsetLeft + element.offsetWidth),
			);
			const pointerX = event.clientX - tabListBounds.left;
			const pointerY = event.clientY - tabListBounds.top;
			applyEvent({
				draggedGroupId: groupId,
				draggedWidth: groupRight - event.currentTarget.offsetLeft,
				grabOffset: pointerX - event.currentTarget.offsetLeft,
				kind: 'arm-group',
				layout: measureIdealLayout(),
				pointerId: event.pointerId,
				pointerX,
				pointerY,
				sourceLeft: event.currentTarget.offsetLeft,
				state: authoritativeState,
			});
		},
		onPointerMove: (event) => {
			const tabListBounds = tabListRef.current?.getBoundingClientRect();
			if (
				tabListBounds !== undefined &&
				movePointer(
					event.pointerId,
					event.clientX - tabListBounds.left,
					event.clientY - tabListBounds.top,
					(event.buttons & 1) === 1,
				)
			)
				event.preventDefault();
		},
		onPointerUp: (event) => {
			if (releasePointer(event.pointerId)) event.preventDefault();
		},
	});

	const getTabDragProperties = (tabId: string): WorkbenchTabDragProperties => ({
		...createSharedProperties(draggedTabId === tabId, onTabMove !== undefined),
		onPointerCancel: (event) => cancelPointer(event.pointerId),
		onPointerDown: (event) => {
			if (onTabMove === undefined || !canArmWorkbenchTabDrag(event)) return;
			const eventTarget = event.target;
			if (
				eventTarget instanceof Element &&
				eventTarget.closest('[data-workbench-tab-action]') !== null
			) {
				return;
			}
			const tabList = tabListRef.current;
			if (tabList === null) return;
			const tabListBounds = tabList.getBoundingClientRect();
			const pointerX = event.clientX - tabListBounds.left;
			const pointerY = event.clientY - tabListBounds.top;
			draggedTabElementRef.current = event.currentTarget;
			applyEvent({
				draggedTabId: tabId,
				draggedWidth: event.currentTarget.offsetWidth,
				grabOffset: pointerX - event.currentTarget.offsetLeft,
				kind: 'arm-tab',
				layout: measureIdealLayout(),
				pointerId: event.pointerId,
				pointerX,
				pointerY,
				sourceLeft: event.currentTarget.offsetLeft,
				state: authoritativeState,
			});
		},
		onPointerMove: (event) => {
			const tabListBounds = tabListRef.current?.getBoundingClientRect();
			if (
				tabListBounds !== undefined &&
				movePointer(
					event.pointerId,
					event.clientX - tabListBounds.left,
					event.clientY - tabListBounds.top,
					(event.buttons & 1) === 1,
				)
			)
				event.preventDefault();
		},
		onPointerUp: (event) => {
			if (releasePointer(event.pointerId)) event.preventDefault();
		},
	});

	return {
		dragOverlayRef,
		dragPlaceholderRef,
		draggedGroupId,
		draggedTabId,
		getTabGroupDragProperties,
		getTabDragProperties,
		hoverSuppressed,
		isReordering,
		onKeyDown: (event) => {
			const currentState = dragStateRef.current;
			if (event.key !== 'Escape' || !hasWorkbenchTabDragActivePointer(currentState)) return;
			event.preventDefault();
			cancelPointer(currentState.pointerId);
		},
		onLostPointerCapture: (event) => {
			if (hasWorkbenchTabDragActivePointer(dragStateRef.current)) cancelPointer(event.pointerId);
		},
		onPointerCancel: (event) => cancelPointer(event.pointerId),
		onPointerMove: (event) => {
			if (hoverSuppressed) setHoverSuppressed(false);
			if (event.target !== event.currentTarget) return;
			const tabListBounds = event.currentTarget.getBoundingClientRect();
			if (
				movePointer(
					event.pointerId,
					event.clientX - tabListBounds.left,
					event.clientY - tabListBounds.top,
					(event.buttons & 1) === 1,
				)
			)
				event.preventDefault();
		},
		onPointerUp: (event) => {
			if (event.target !== event.currentTarget) return;
			if (releasePointer(event.pointerId)) event.preventDefault();
		},
		tabGroups: displayState.tabGroups,
		tabListRef,
		tabs: displayState.tabs,
	};
}
