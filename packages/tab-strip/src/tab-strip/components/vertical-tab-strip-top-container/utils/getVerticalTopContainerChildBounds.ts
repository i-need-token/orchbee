import type { calculateVerticalTopContainerLayout } from '../../../../vertical';

export function getVerticalTopContainerChildBounds(
	layout: ReturnType<typeof calculateVerticalTopContainerLayout>,
	id: 'collapse' | 'unfocus' | 'combo',
): ReturnType<typeof calculateVerticalTopContainerLayout>['children'][number]['bounds'] | null {
	return layout.children.find((child) => child.id === id)?.bounds ?? null;
}
