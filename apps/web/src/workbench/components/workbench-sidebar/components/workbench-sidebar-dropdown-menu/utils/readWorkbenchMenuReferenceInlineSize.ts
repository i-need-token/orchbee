export function readWorkbenchMenuReferenceInlineSize(ownerDocument: Document): number | null {
	const ownerWindow = ownerDocument.defaultView;
	if (ownerWindow === null) {
		return null;
	}

	const rootStyles = ownerWindow.getComputedStyle(ownerDocument.documentElement);
	const tokenValue = rootStyles.getPropertyValue('--ob-ref-size-menu').trim();
	const numericValue = Number.parseFloat(tokenValue);
	if (!Number.isFinite(numericValue) || numericValue <= 0) {
		return null;
	}

	if (tokenValue.endsWith('px')) {
		return numericValue;
	}
	if (tokenValue.endsWith('rem')) {
		const rootFontSize = Number.parseFloat(rootStyles.fontSize);
		return Number.isFinite(rootFontSize) && rootFontSize > 0 ? numericValue * rootFontSize : null;
	}
	return null;
}
