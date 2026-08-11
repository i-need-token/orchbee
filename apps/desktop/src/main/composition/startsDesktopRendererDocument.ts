export interface DesktopRendererNavigation {
	readonly isMainFrame: boolean;
	readonly isSameDocument: boolean;
}

export function startsDesktopRendererDocument(navigation: DesktopRendererNavigation): boolean {
	return navigation.isMainFrame && !navigation.isSameDocument;
}
