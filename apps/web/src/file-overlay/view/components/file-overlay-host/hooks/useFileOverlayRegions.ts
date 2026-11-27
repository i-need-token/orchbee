import * as React from 'react';

import type { FileOverlayRegionElements } from '../../../file-overlay-contract';
import type { FileOverlayRegions } from '../../../../model';

function sameRegions(left: FileOverlayRegions, right: FileOverlayRegions): boolean {
	return (
		left.width === right.width &&
		left.terminal.left === right.terminal.left &&
		left.terminal.top === right.terminal.top &&
		left.terminal.width === right.terminal.width &&
		left.terminal.height === right.terminal.height &&
		left.sidebar?.left === right.sidebar?.left &&
		left.sidebar?.right === right.sidebar?.right
	);
}

/**
 * Measures what the overlay lies over before it paints, and again whenever the window or the
 * terminal area changes size, while `active`. Moving or resizing any column resizes the terminal
 * area, so observing it with the root sees every layout change. Null until the elements exist.
 */
export function useFileOverlayRegions(
	elements: FileOverlayRegionElements,
	sidebarDocked: boolean,
	active: boolean,
): FileOverlayRegions | null {
	const [regions, setRegions] = React.useState<FileOverlayRegions | null>(null);
	React.useLayoutEffect(() => {
		if (!active) return undefined;
		const root = elements.root.current;
		const terminalArea = elements.terminalArea.current;
		if (root === null || terminalArea === null) return undefined;
		const measure = (): void => {
			const origin = root.getBoundingClientRect();
			const area = terminalArea.getBoundingClientRect();
			const sidebar = sidebarDocked
				? (elements.sidebar.current?.getBoundingClientRect() ?? null)
				: null;
			const measured: FileOverlayRegions = {
				sidebar:
					sidebar === null
						? null
						: { left: sidebar.left - origin.left, right: sidebar.right - origin.left },
				terminal: {
					height: area.height,
					left: area.left - origin.left,
					top: area.top - origin.top,
					width: area.width,
				},
				width: origin.width,
			};
			setRegions((current) =>
				current !== null && sameRegions(current, measured) ? current : measured,
			);
		};
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(root);
		observer.observe(terminalArea);
		return () => observer.disconnect();
	}, [active, elements, sidebarDocked]);
	return regions;
}
