import type { calculateVerticalTopContainerLayout } from './vertical-tab-strip-top-container-layout';

export function getVerticalTopContainerChildBounds(
	layout: ReturnType<typeof calculateVerticalTopContainerLayout>,
	id: 'collapse' | 'combo',
): ReturnType<typeof calculateVerticalTopContainerLayout>['children'][number]['bounds'] | null {
	return layout.children.find((child) => child.id === id)?.bounds ?? null;
}
