// Ported from chrome/browser/ui/tabs/tab_types.h.

/** Whether a tab participates in layout as an open or closing/closed tab. */
export const TabOpen = {
	Open: 'open',
	Closed: 'closed',
} as const;

export type TabOpen = (typeof TabOpen)[keyof typeof TabOpen];

/** Whether a tab belongs to the pinned portion of the strip. */
export const TabPinned = {
	Pinned: 'pinned',
	Unpinned: 'unpinned',
} as const;

export type TabPinned = (typeof TabPinned)[keyof typeof TabPinned];

/** Whether a tab is active. */
export const TabActive = {
	Active: 'active',
	Inactive: 'inactive',
} as const;

export type TabActive = (typeof TabActive)[keyof typeof TabActive];

/** Opaque identifiers deliberately match Chromium's strongly typed IDs. */
export type TabGroupId = string;
