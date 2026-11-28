/**
 * How much of the window the open overlay covers below the 36px row: the terminal area, the Sidebar
 * and the terminal area, or the whole width.
 */
export type FileOverlayTier = 'default' | 'full' | 'left-expand';

/** The tiers below full, which Restore returns to and the frame's left edge chooses between. */
export type FileOverlayNarrowTier = Exclude<FileOverlayTier, 'full'>;

/**
 * A Workspace's overlay: closed, open at its tier, or shrunk to picture-in-picture. Closed and
 * picture-in-picture keep the tier it opens at next, so it opens at the tier last used.
 */
export interface FileOverlayMode {
	readonly kind: 'closed' | 'open' | 'pip';
	/**
	 * Whether the window offers left-expand: the Sidebar shows docked and the File Bar is on the
	 * right. Neither tier field names left-expand while it is not offered.
	 */
	readonly leftExpandOffered: boolean;
	/** Where Restore returns while the tier is full. */
	readonly restoreTier: FileOverlayNarrowTier;
	readonly tier: FileOverlayTier;
}

/** What changes the overlay's mode or tier rather than its tabs. */
export type FileOverlayModeEvent =
	/** The scrim was clicked, or an Esc reached the overlay with nothing else consuming it. */
	| { readonly type: 'dismissed' }
	/** Expand or Restore at the end of the tab row, or `Mod+K Mod+M`. */
	| { readonly type: 'full-toggled' }
	| { readonly leftExpandOffered: boolean; readonly type: 'layout-changed' }
	/** Picture-in-picture was clicked, or activated with Enter or Space. */
	| { readonly type: 'pip-restored' }
	/**
	 * Something replaced what the terminal area showed behind the overlay: another Tab or Workspace,
	 * focus moving into a region the overlay covers, or an Action writing into the current terminal.
	 */
	| { readonly type: 'terminal-replaced' }
	/** The frame's left edge was released over a tier. */
	| { readonly tier: FileOverlayNarrowTier; readonly type: 'tier-chosen' };

export function createFileOverlayMode(leftExpandOffered: boolean): FileOverlayMode {
	return { kind: 'closed', leftExpandOffered, restoreTier: 'default', tier: 'default' };
}

/** Shows a closed or picture-in-picture overlay at its remembered tier, as opening a file does. */
export function openFileOverlayMode(mode: FileOverlayMode): FileOverlayMode {
	return mode.kind === 'open' ? mode : { ...mode, kind: 'open' };
}

/** Closes the overlay once its last tab has closed; the tier stays for the next open. */
export function closeFileOverlayMode(mode: FileOverlayMode): FileOverlayMode {
	return mode.kind === 'closed' ? mode : { ...mode, kind: 'closed' };
}

/**
 * Dismissing the overlay or replacing the terminal behind it shrinks it to picture-in-picture, and
 * restoring that shows it again at its tier. Tier changes apply only to the open overlay, whose row
 * shows Expand and Restore and whose frame has the left edge. A tier that stops being offered falls
 * back to default, so Restore then returns to default.
 */
export function transitionFileOverlayMode(
	mode: FileOverlayMode,
	event: FileOverlayModeEvent,
): FileOverlayMode {
	switch (event.type) {
		case 'dismissed':
		case 'terminal-replaced':
			return mode.kind === 'open' ? { ...mode, kind: 'pip' } : mode;
		case 'pip-restored':
			return mode.kind === 'pip' ? { ...mode, kind: 'open' } : mode;
		case 'full-toggled':
			if (mode.kind !== 'open') return mode;
			return mode.tier === 'full'
				? { ...mode, tier: mode.restoreTier }
				: { ...mode, restoreTier: mode.tier, tier: 'full' };
		case 'tier-chosen':
			if (mode.kind !== 'open' || mode.tier === 'full' || mode.tier === event.tier) return mode;
			if (event.tier === 'left-expand' && !mode.leftExpandOffered) return mode;
			return { ...mode, tier: event.tier };
		case 'layout-changed':
			if (event.leftExpandOffered === mode.leftExpandOffered) return mode;
			if (event.leftExpandOffered) return { ...mode, leftExpandOffered: true };
			return {
				...mode,
				leftExpandOffered: false,
				restoreTier: 'default',
				tier: mode.tier === 'left-expand' ? 'default' : mode.tier,
			};
	}
}
