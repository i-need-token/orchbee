/** The File Bar's views, in the order the toggles row shows them. */
export const FILE_BAR_VIEW_IDS = ['explorer', 'source-control'] as const;

export type FileBarViewId = (typeof FILE_BAR_VIEW_IDS)[number];

/**
 * The side of the terminal area the File Bar takes, from the user's settings: after it by default,
 * or between the Sidebar and it.
 */
export type FileBarPosition = 'left' | 'right';

/** The File Bar column's widths in pixels. */
export const FILE_BAR_WIDTH = {
	default: 300,
	maximum: 560,
	minimum: 240,
	/** Where the window draws its caption buttons over the File Bar's row, so the toggles still fit. */
	minimumBesideCaptionButtons: 280,
} as const;

/** What the File Bar keeps on this device: whether it shows docked, how wide, and which view. */
export interface FileBarLayout {
	readonly activeView: FileBarViewId;
	readonly visible: boolean;
	readonly width: number;
}

/** A File Bar never committed starts hidden, so a person who never shows it pays nothing. */
export const DEFAULT_FILE_BAR_LAYOUT: FileBarLayout = {
	activeView: 'explorer',
	visible: false,
	width: FILE_BAR_WIDTH.default,
};
