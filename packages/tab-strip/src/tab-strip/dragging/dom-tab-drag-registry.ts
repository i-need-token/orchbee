import { requireDefined } from '../utils/requireDefined';
import type { Rect } from '../utils/rectRight';
import type { TabGroupId } from '../model/tab-types';
import type { DragSessionData } from './drag-session-data';
import type { CrossWindowDragHost } from './cross-window-drag-runtime';
import type { DragPoint } from './dragging-tabs-session';
import type { TabDragContext } from './tab-drag-context';
import type { TabDragPointResolver, TabDragTarget } from './tab-drag-target';
export const MOUSE_VERTICAL_DETACH_MAGNETISM = 15;
export const TOUCH_VERTICAL_DETACH_MAGNETISM = 50;
export interface DomTabDragBrowserRegistration {
	browserId: string;
	context: TabDragContext;
	getWindowBounds: () => Readonly<Rect>;
	getTabStripBounds: () => Readonly<Rect>;
	getDragTarget?: (pointInScreen: Readonly<DragPoint>) => TabDragTarget | null;
	getInsertionIndex?: (pointInScreen: Readonly<DragPoint>, sessionData: DragSessionData) => number;
	getTargetGroup?: (
		pointInScreen: Readonly<DragPoint>,
		sessionData: DragSessionData,
	) => TabGroupId | null | undefined;
	touch?: boolean;
}
export interface DomTabDragRegistryOptions {
	createBrowserContext?: (
		pointInScreen: Readonly<DragPoint>,
		sessionData: DragSessionData,
	) => TabDragContext | null;
	onContextEmptied?: (context: TabDragContext) => void;
}
function contains(rect: Readonly<Rect>, point: Readonly<DragPoint>): boolean {
	return (
		point.x >= rect.x &&
		point.x < rect.x + rect.width &&
		point.y >= rect.y &&
		point.y < rect.y + rect.height
	);
}

/**
 * Screen-coordinate browser discovery for DOM/Electron/popup hosts. Newer
 * registrations win, matching a topmost-window traversal.
 */
export class DomTabDragRegistry implements CrossWindowDragHost, TabDragPointResolver {
	private readonly registrations: DomTabDragBrowserRegistration[] = [];
	private readonly options: DomTabDragRegistryOptions;
	public constructor(options: DomTabDragRegistryOptions = {}) {
		this.options = options;
	}
	public registerBrowser(registration: DomTabDragBrowserRegistration): () => void {
		if (this.registrations.some((candidate) => candidate.browserId === registration.browserId)) {
			throw new Error(`Browser ${registration.browserId} is already registered.`);
		}
		this.registrations.push(registration);
		return () => {
			const index = this.registrations.indexOf(registration);
			if (index !== -1) this.registrations.splice(index, 1);
		};
	}
	public getBrowserIdAtPoint(pointInScreen: Readonly<DragPoint>): string | null {
		return this.getRegistrationAtPoint(pointInScreen)?.browserId ?? null;
	}
	public getContextAtPoint(
		browserId: string,
		pointInScreen: Readonly<DragPoint>,
	): TabDragContext | null {
		const registration = this.registrations.find((candidate) => candidate.browserId === browserId);
		if (!registration) return null;
		const bounds = registration.getTabStripBounds();
		const delegate = registration.context.getPositioningDelegate();
		const begin = bounds.x + (delegate?.tabDragAreaBeginX() ?? 0);
		const end = bounds.x + (delegate?.tabDragAreaEndX() ?? bounds.width);
		const magnetism = registration.touch
			? TOUCH_VERTICAL_DETACH_MAGNETISM
			: MOUSE_VERTICAL_DETACH_MAGNETISM;
		const verticallyContained =
			pointInScreen.y >= bounds.y - magnetism &&
			pointInScreen.y < bounds.y + bounds.height + magnetism;
		return pointInScreen.x >= begin && pointInScreen.x < end && verticallyContained
			? registration.context
			: null;
	}
	public getDragTarget(
		browserId: string,
		pointInScreen: Readonly<DragPoint>,
	): TabDragTarget | null {
		const registration = this.registrations.find((candidate) => candidate.browserId === browserId);
		if (!registration || this.getContextAtPoint(browserId, pointInScreen)) return null;
		return registration.getDragTarget?.(pointInScreen) ?? null;
	}
	public createBrowserContext(
		pointInScreen: Readonly<DragPoint>,
		sessionData: DragSessionData,
	): TabDragContext | null {
		return this.options.createBrowserContext?.(pointInScreen, sessionData) ?? null;
	}
	public getInsertionIndex(
		context: TabDragContext,
		pointInScreen: Readonly<DragPoint>,
		sessionData: DragSessionData,
	): number {
		const registration = this.registrations.find((candidate) => candidate.context === context);
		return (
			registration?.getInsertionIndex?.(pointInScreen, sessionData) ??
			context.getTabStripModel().getSnapshot().tabs.length
		);
	}
	public getTargetGroup(
		context: TabDragContext,
		pointInScreen: Readonly<DragPoint>,
		sessionData: DragSessionData,
	): TabGroupId | null | undefined {
		return this.registrations
			.find((candidate) => candidate.context === context)
			?.getTargetGroup?.(pointInScreen, sessionData);
	}
	public onContextEmptied(context: TabDragContext): void {
		this.options.onContextEmptied?.(context);
	}
	private getRegistrationAtPoint(
		pointInScreen: Readonly<DragPoint>,
	): DomTabDragBrowserRegistration | null {
		for (let index = this.registrations.length - 1; index >= 0; index -= 1) {
			const registration = requireDefined(this.registrations[index]);
			if (contains(registration.getWindowBounds(), pointInScreen)) return registration;
		}
		return null;
	}
}

/** Converts a viewport DOMRect to the shared screen coordinate space. */
export function domRectToScreenRect(
	rect: Readonly<DOMRect>,
	ownerWindow: Pick<Window, 'screenX' | 'screenY'>,
): Rect {
	return {
		x: ownerWindow.screenX + rect.left,
		y: ownerWindow.screenY + rect.top,
		width: rect.width,
		height: rect.height,
	};
}
