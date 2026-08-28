import { z } from 'zod';

import { createDesktopWindowState, type DesktopWindowBounds } from '../window-state';

const DEFAULT_WINDOW_BOUNDS = {
	height: 720,
	width: 1_024,
	x: 0,
	y: 0,
} satisfies DesktopWindowBounds;
const MAX_ASSIGNMENTS_PER_WINDOW = 128;
const MAX_IDENTIFIER_LENGTH = 256;
const MAX_TABS_PER_CONTEXT = 4_096;
const MAX_WINDOWS = 64;
const MAX_WINDOW_SEQUENCE = 1_000_000;

const identifierSchema = z
	.string()
	.min(1)
	.max(MAX_IDENTIFIER_LENGTH)
	.regex(/^[A-Za-z0-9:._-]+$/u);
const boundsSchema = z
	.strictObject({
		height: z.number(),
		width: z.number(),
		x: z.number(),
		y: z.number(),
	})
	.refine((bounds) => createDesktopWindowState(bounds, false) !== null);
const assignmentSchema = z.strictObject({
	tabIds: z.array(identifierSchema).max(MAX_TABS_PER_CONTEXT),
	terminalContextId: identifierSchema,
});
const windowSchema = z.strictObject({
	assignments: z.array(assignmentSchema).max(MAX_ASSIGNMENTS_PER_WINDOW),
	bounds: boundsSchema,
	maximized: z.boolean(),
	ordinal: z.number().int().min(1).max(MAX_WINDOW_SEQUENCE),
	windowId: identifierSchema,
});
const layoutSchema = z.strictObject({
	activationOrder: z.array(identifierSchema).min(1).max(MAX_WINDOWS),
	nextWindowSequence: z.number().int().min(1).max(MAX_WINDOW_SEQUENCE),
	primaryWindowId: identifierSchema,
	version: z.literal(1),
	windows: z.array(windowSchema).min(1).max(MAX_WINDOWS),
});

interface DesktopWindowLayoutAssignment {
	readonly tabIds: readonly string[];
	readonly terminalContextId: string;
}

export interface DesktopWindowLayoutDocument {
	readonly terminalContexts: readonly {
		readonly tabIds: readonly string[];
		readonly terminalContextId: string;
	}[];
}

export interface DesktopWindowLayoutWindow {
	readonly assignments: readonly DesktopWindowLayoutAssignment[];
	readonly bounds: DesktopWindowBounds;
	readonly maximized: boolean;
	readonly ordinal: number;
	readonly windowId: string;
}

export interface DesktopWindowLayout {
	readonly activationOrder: readonly string[];
	readonly nextWindowSequence: number;
	readonly primaryWindowId: string;
	readonly version: 1;
	readonly windows: readonly DesktopWindowLayoutWindow[];
}

export type DesktopWindowLayoutLoadResult =
	| { readonly kind: 'available'; readonly layout: DesktopWindowLayout }
	| { readonly kind: 'missing' }
	| { readonly kind: 'unavailable'; readonly reason: 'invalid' | 'read-failed' };

export interface DesktopWindowLayoutStore {
	flush(): Promise<void>;
	load(): Promise<DesktopWindowLayoutLoadResult>;
	save(layout: DesktopWindowLayout): Promise<boolean>;
}

export interface DesktopWindowLayoutNormalization {
	readonly changed: boolean;
	readonly kind: 'reconciled' | 'recovered';
	readonly layout: DesktopWindowLayout;
}

export type DesktopWindowLayoutMoveDestination =
	| { readonly kind: 'existing'; readonly windowId: string }
	| { readonly bounds: DesktopWindowBounds; readonly kind: 'new' };

export type DesktopWindowLayoutMoveResult =
	| {
			readonly closedWindowIds: readonly string[];
			readonly destinationWindowId: string;
			readonly kind: 'completed';
			readonly layout: DesktopWindowLayout;
	  }
	| { readonly kind: 'unchanged'; readonly reason: 'already-assigned' }
	| {
			readonly kind: 'blocked';
			readonly reason: 'invalid-bounds' | 'stale-tab' | 'stale-window' | 'window-capacity';
	  };

export type DesktopWindowLayoutCloseResult =
	| {
			readonly destinationWindowId: string;
			readonly kind: 'completed';
			readonly layout: DesktopWindowLayout;
	  }
	| { readonly kind: 'last-window' }
	| { readonly kind: 'blocked'; readonly reason: 'stale-window' };

export type DesktopWindowLayoutFocusResult =
	| { readonly kind: 'completed'; readonly layout: DesktopWindowLayout }
	| { readonly kind: 'unchanged' }
	| { readonly kind: 'blocked'; readonly reason: 'stale-window' };

export type DesktopWindowLayoutGeometryResult =
	| { readonly kind: 'completed'; readonly layout: DesktopWindowLayout }
	| { readonly kind: 'unchanged' }
	| { readonly kind: 'blocked'; readonly reason: 'invalid-bounds' | 'stale-window' };

export function parseDesktopWindowLayout(value: unknown): DesktopWindowLayout | null {
	const parsed = layoutSchema.safeParse(value);
	return parsed.success ? parsed.data : null;
}

export function normalizeDesktopWindowLayout(
	layout: DesktopWindowLayout | null,
	document: DesktopWindowLayoutDocument,
	fallbackBounds: DesktopWindowBounds = DEFAULT_WINDOW_BOUNDS,
	fallbackMaximized = false,
): DesktopWindowLayoutNormalization {
	const canonicalDocument = canonicalizeDocument(document);
	if (!isValidBounds(fallbackBounds) || layout === null || !hasValidLayoutIdentity(layout)) {
		return {
			changed: true,
			kind: 'recovered',
			layout: createCollapsedLayout(canonicalDocument, fallbackBounds, fallbackMaximized),
		};
	}

	const currentTabIds = new Set(
		canonicalDocument.terminalContexts.flatMap((context) => context.tabIds),
	);
	const assignedTabIds = new Set<string>();
	for (const window of layout.windows) {
		for (const assignment of window.assignments) {
			for (const tabId of assignment.tabIds) {
				if (!currentTabIds.has(tabId)) continue;
				if (assignedTabIds.has(tabId)) {
					return {
						changed: true,
						kind: 'recovered',
						layout: createCollapsedLayout(canonicalDocument, fallbackBounds, fallbackMaximized),
					};
				}
				assignedTabIds.add(tabId);
			}
		}
	}

	const recentWindowId = layout.activationOrder.at(-1) ?? layout.primaryWindowId;
	const tabOwners = collectTabOwners(layout, currentTabIds);
	for (const tabId of currentTabIds) {
		if (!tabOwners.has(tabId)) tabOwners.set(tabId, recentWindowId);
	}
	const canonical = rebuildLayout(layout, canonicalDocument, tabOwners);
	return {
		changed: serializeLayout(canonical) !== serializeLayout(layout),
		kind: 'reconciled',
		layout: canonical,
	};
}

export function focusDesktopWindow(
	layout: DesktopWindowLayout,
	windowId: string,
): DesktopWindowLayoutFocusResult {
	if (!layout.windows.some((window) => window.windowId === windowId)) {
		return { kind: 'blocked', reason: 'stale-window' };
	}
	if (layout.activationOrder.at(-1) === windowId) return { kind: 'unchanged' };
	return {
		kind: 'completed',
		layout: {
			...layout,
			activationOrder: [...layout.activationOrder.filter((id) => id !== windowId), windowId],
		},
	};
}

export function moveDesktopWindowTabs(
	layout: DesktopWindowLayout,
	document: DesktopWindowLayoutDocument,
	tabIds: readonly string[],
	destination: DesktopWindowLayoutMoveDestination,
): DesktopWindowLayoutMoveResult {
	const canonicalDocument = canonicalizeDocument(document);
	const documentTabIds = new Set(
		canonicalDocument.terminalContexts.flatMap((context) => context.tabIds),
	);
	const uniqueTabIds = [...new Set(tabIds)];
	if (
		uniqueTabIds.length === 0 ||
		uniqueTabIds.length !== tabIds.length ||
		uniqueTabIds.some((tabId) => !documentTabIds.has(tabId))
	) {
		return { kind: 'blocked', reason: 'stale-tab' };
	}
	const normalized = normalizeDesktopWindowLayout(layout, canonicalDocument);
	const current = normalized.layout;
	const tabOwners = collectTabOwners(current, documentTabIds);
	let destinationWindowId: string;
	let windows = [...current.windows];
	let nextWindowSequence = current.nextWindowSequence;
	if (destination.kind === 'existing') {
		if (!windows.some((window) => window.windowId === destination.windowId)) {
			return { kind: 'blocked', reason: 'stale-window' };
		}
		destinationWindowId = destination.windowId;
	} else {
		if (!isValidBounds(destination.bounds)) {
			return { kind: 'blocked', reason: 'invalid-bounds' };
		}
		if (windows.length >= MAX_WINDOWS || current.nextWindowSequence >= MAX_WINDOW_SEQUENCE) {
			return { kind: 'blocked', reason: 'window-capacity' };
		}
		const allocated = allocateWindowId(current);
		destinationWindowId = allocated.windowId;
		nextWindowSequence = allocated.nextWindowSequence;
		windows.push({
			assignments: [],
			bounds: destination.bounds,
			maximized: false,
			ordinal: allocated.ordinal,
			windowId: allocated.windowId,
		});
	}
	if (uniqueTabIds.every((tabId) => tabOwners.get(tabId) === destinationWindowId)) {
		return { kind: 'unchanged', reason: 'already-assigned' };
	}

	const sourceWindowIds = new Set(
		uniqueTabIds.flatMap((tabId) => {
			const owner = tabOwners.get(tabId);
			return owner === undefined || owner === destinationWindowId ? [] : [owner];
		}),
	);
	for (const tabId of uniqueTabIds) tabOwners.set(tabId, destinationWindowId);
	const emptiedSourceIds = [...sourceWindowIds].filter(
		(sourceWindowId) =>
			![...documentTabIds].some((tabId) => tabOwners.get(tabId) === sourceWindowId),
	);
	windows = windows.filter((window) => !emptiedSourceIds.includes(window.windowId));
	const primaryWindowId = emptiedSourceIds.includes(current.primaryWindowId)
		? destinationWindowId
		: current.primaryWindowId;
	const base: DesktopWindowLayout = {
		activationOrder: [
			...current.activationOrder.filter(
				(windowId) => !emptiedSourceIds.includes(windowId) && windowId !== destinationWindowId,
			),
			destinationWindowId,
		],
		nextWindowSequence,
		primaryWindowId,
		version: 1,
		windows,
	};
	return {
		closedWindowIds: emptiedSourceIds,
		destinationWindowId,
		kind: 'completed',
		layout: rebuildLayout(base, canonicalDocument, tabOwners),
	};
}

export function closeDesktopWindow(
	layout: DesktopWindowLayout,
	document: DesktopWindowLayoutDocument,
	windowId: string,
): DesktopWindowLayoutCloseResult {
	const canonicalDocument = canonicalizeDocument(document);
	const current = normalizeDesktopWindowLayout(layout, canonicalDocument).layout;
	if (!current.windows.some((window) => window.windowId === windowId)) {
		return { kind: 'blocked', reason: 'stale-window' };
	}
	if (current.windows.length === 1) return { kind: 'last-window' };
	const destinationWindowId = [...current.activationOrder]
		.reverse()
		.find((candidate) => candidate !== windowId);
	if (destinationWindowId === undefined) return { kind: 'blocked', reason: 'stale-window' };
	const documentTabIds = new Set(
		canonicalDocument.terminalContexts.flatMap((context) => context.tabIds),
	);
	const tabOwners = collectTabOwners(current, documentTabIds);
	for (const [tabId, ownerWindowId] of tabOwners) {
		if (ownerWindowId === windowId) tabOwners.set(tabId, destinationWindowId);
	}
	const base: DesktopWindowLayout = {
		activationOrder: [
			...current.activationOrder.filter(
				(candidate) => candidate !== windowId && candidate !== destinationWindowId,
			),
			destinationWindowId,
		],
		nextWindowSequence: current.nextWindowSequence,
		primaryWindowId:
			current.primaryWindowId === windowId ? destinationWindowId : current.primaryWindowId,
		version: 1,
		windows: current.windows.filter((window) => window.windowId !== windowId),
	};
	return {
		destinationWindowId,
		kind: 'completed',
		layout: rebuildLayout(base, canonicalDocument, tabOwners),
	};
}

export function updateDesktopWindowGeometry(
	layout: DesktopWindowLayout,
	windowId: string,
	bounds: DesktopWindowBounds,
	maximized: boolean,
): DesktopWindowLayoutGeometryResult {
	if (!isValidBounds(bounds)) return { kind: 'blocked', reason: 'invalid-bounds' };
	const target = layout.windows.find((window) => window.windowId === windowId);
	if (target === undefined) return { kind: 'blocked', reason: 'stale-window' };
	if (
		serializeLayout(target.bounds) === serializeLayout(bounds) &&
		target.maximized === maximized
	) {
		return { kind: 'unchanged' };
	}
	return {
		kind: 'completed',
		layout: {
			...layout,
			windows: layout.windows.map((window) =>
				window.windowId === windowId ? { ...window, bounds, maximized } : window,
			),
		},
	};
}

export function inspectDesktopWindowLayoutInvariantViolations(
	layout: DesktopWindowLayout,
	document: DesktopWindowLayoutDocument,
): readonly string[] {
	const violations: string[] = [];
	if (!hasValidLayoutIdentity(layout)) violations.push('layout-identity');
	const canonicalDocument = canonicalizeDocument(document);
	const expectedTabIds = canonicalDocument.terminalContexts.flatMap((context) => context.tabIds);
	const actualTabIds = layout.windows.flatMap((window) =>
		window.assignments.flatMap((assignment) => assignment.tabIds),
	);
	if (new Set(actualTabIds).size !== actualTabIds.length) violations.push('duplicate-tab');
	if (
		expectedTabIds.length !== actualTabIds.length ||
		expectedTabIds.some((tabId) => !actualTabIds.includes(tabId))
	) {
		violations.push('incomplete-tab-partition');
	}
	const canonical = normalizeDesktopWindowLayout(layout, canonicalDocument);
	if (canonical.kind === 'recovered' || canonical.changed) violations.push('non-canonical-layout');
	return violations;
}

function allocateWindowId(layout: DesktopWindowLayout): {
	readonly nextWindowSequence: number;
	readonly ordinal: number;
	readonly windowId: string;
} {
	const existingIds = new Set(layout.windows.map((window) => window.windowId));
	let ordinal = layout.nextWindowSequence;
	let windowId = `desktop-window-${ordinal}`;
	while (existingIds.has(windowId) && ordinal < MAX_WINDOW_SEQUENCE) {
		ordinal += 1;
		windowId = `desktop-window-${ordinal}`;
	}
	return {
		nextWindowSequence: Math.min(ordinal + 1, MAX_WINDOW_SEQUENCE),
		ordinal,
		windowId,
	};
}

function canonicalizeDocument(document: DesktopWindowLayoutDocument): DesktopWindowLayoutDocument {
	const seenTabIds = new Set<string>();
	return {
		terminalContexts: document.terminalContexts.map((context) => ({
			tabIds: context.tabIds.filter((tabId) => {
				if (seenTabIds.has(tabId)) return false;
				seenTabIds.add(tabId);
				return true;
			}),
			terminalContextId: context.terminalContextId,
		})),
	};
}

function collectTabOwners(
	layout: DesktopWindowLayout,
	currentTabIds: ReadonlySet<string>,
): Map<string, string> {
	const owners = new Map<string, string>();
	for (const window of layout.windows) {
		for (const assignment of window.assignments) {
			for (const tabId of assignment.tabIds) {
				if (currentTabIds.has(tabId) && !owners.has(tabId)) owners.set(tabId, window.windowId);
			}
		}
	}
	return owners;
}

function createCollapsedLayout(
	document: DesktopWindowLayoutDocument,
	bounds: DesktopWindowBounds,
	maximized: boolean,
): DesktopWindowLayout {
	const windowId = 'desktop-window-1';
	return {
		activationOrder: [windowId],
		nextWindowSequence: 2,
		primaryWindowId: windowId,
		version: 1,
		windows: [
			{
				assignments: document.terminalContexts
					.filter((context) => context.tabIds.length > 0)
					.map((context) => ({
						tabIds: [...context.tabIds],
						terminalContextId: context.terminalContextId,
					})),
				bounds,
				maximized,
				ordinal: 1,
				windowId,
			},
		],
	};
}

function hasValidLayoutIdentity(layout: DesktopWindowLayout): boolean {
	const windowIds = layout.windows.map((window) => window.windowId);
	const ordinals = layout.windows.map((window) => window.ordinal);
	const activationIds = layout.activationOrder;
	return (
		new Set(windowIds).size === windowIds.length &&
		new Set(ordinals).size === ordinals.length &&
		new Set(activationIds).size === activationIds.length &&
		activationIds.length === windowIds.length &&
		windowIds.every((windowId) => activationIds.includes(windowId)) &&
		windowIds.includes(layout.primaryWindowId) &&
		layout.nextWindowSequence > Math.max(...ordinals)
	);
}

function isValidBounds(bounds: DesktopWindowBounds): boolean {
	return createDesktopWindowState(bounds, false) !== null;
}

function rebuildLayout(
	layout: DesktopWindowLayout,
	document: DesktopWindowLayoutDocument,
	tabOwners: ReadonlyMap<string, string>,
): DesktopWindowLayout {
	return {
		...layout,
		windows: [...layout.windows]
			.sort((left, right) => left.ordinal - right.ordinal)
			.map((window) => ({
				...window,
				assignments: document.terminalContexts.flatMap((context) => {
					const tabIds = context.tabIds.filter((tabId) => tabOwners.get(tabId) === window.windowId);
					return tabIds.length === 0
						? []
						: [{ tabIds, terminalContextId: context.terminalContextId }];
				}),
			})),
	};
}

function serializeLayout(value: unknown): string {
	return JSON.stringify(value);
}
