import type { calculateVerticalTopContainerLayout } from './calculateVerticalTopContainerLayout';

export function getVerticalTopContainerChildBounds(
	layout: ReturnType<typeof calculateVerticalTopContainerLayout>,
	id: 'collapse' | 'combo',
): ReturnType<typeof calculateVerticalTopContainerLayout>['children'][number]['bounds'] | null {
	return layout.children.find((child) => child.id === id)?.bounds ?? null;
}
