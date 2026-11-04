interface PanelResizePoint {
	readonly x: number;
	readonly y: number;
}

interface PanelSeparatorBounds {
	readonly height: number;
	readonly left: number;
	readonly top: number;
	readonly width: number;
}

export function getPanelSeparatorIndexAtPoint(
	orientation: 'horizontal' | 'vertical',
	point: PanelResizePoint,
	separatorBounds: readonly PanelSeparatorBounds[],
	minimumTargetSize: number,
): number | null {
	let closestDistance = Number.POSITIVE_INFINITY;
	let closestIndex: number | null = null;

	for (const [index, bounds] of separatorBounds.entries()) {
		const targetWidth = Math.max(bounds.width, minimumTargetSize);
		const targetHeight = Math.max(bounds.height, minimumTargetSize);
		const centerX = bounds.left + bounds.width / 2;
		const centerY = bounds.top + bounds.height / 2;
		const containsPoint =
			point.x >= centerX - targetWidth / 2 &&
			point.x <= centerX + targetWidth / 2 &&
			point.y >= centerY - targetHeight / 2 &&
			point.y <= centerY + targetHeight / 2;
		if (!containsPoint) {
			continue;
		}
		const distance = Math.abs(orientation === 'horizontal' ? point.x - centerX : point.y - centerY);
		if (distance <= closestDistance) {
			closestDistance = distance;
			closestIndex = index;
		}
	}

	return closestIndex;
}
