import type { CSSProperties } from 'react';
import type { calculateVerticalTopContainerLayout } from '../../../../../vertical';

export function getVerticalTopContainerChildStyle(
	layout: ReturnType<typeof calculateVerticalTopContainerLayout>,
	id: 'collapse' | 'unfocus' | 'combo',
): CSSProperties {
	const bounds = layout.children.find((child) => child.id === id)?.bounds;
	if (!bounds) return { display: 'none' };
	return { left: bounds.x, top: bounds.y, width: bounds.width, height: bounds.height };
}
