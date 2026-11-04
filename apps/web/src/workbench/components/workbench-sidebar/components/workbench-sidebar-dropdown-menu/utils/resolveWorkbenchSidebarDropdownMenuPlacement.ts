export type WorkbenchSidebarDropdownMenuPlacement = 'bottom-end' | 'right-start';

interface ResolveWorkbenchSidebarDropdownMenuPlacementOptions {
	readonly menuInlineSize: number | null;
	readonly triggerInlineEnd: number;
	readonly viewportInlineSize: number;
}

export function resolveWorkbenchSidebarDropdownMenuPlacement({
	menuInlineSize,
	triggerInlineEnd,
	viewportInlineSize,
}: ResolveWorkbenchSidebarDropdownMenuPlacementOptions): WorkbenchSidebarDropdownMenuPlacement {
	if (menuInlineSize === null) {
		return 'right-start';
	}

	const availableInlineEnd = viewportInlineSize - triggerInlineEnd;
	// Matches the shared menu primitive's 4px side offset and 5px viewport collision padding.
	const menuGapAndViewportClearance = 9;
	return availableInlineEnd >= menuInlineSize + menuGapAndViewportClearance
		? 'right-start'
		: 'bottom-end';
}
