import add from '@vscode/codicons/src/icons/add.svg?raw';
import chevronRight from '@vscode/codicons/src/icons/chevron-right.svg?raw';
import close from '@vscode/codicons/src/icons/close.svg?raw';
import ellipsis from '@vscode/codicons/src/icons/ellipsis.svg?raw';
import layoutSidebarLeft from '@vscode/codicons/src/icons/layout-sidebar-left.svg?raw';
import settingsGear from '@vscode/codicons/src/icons/settings-gear.svg?raw';
import terminal from '@vscode/codicons/src/icons/terminal.svg?raw';
import type { ReactElement } from 'react';

import styles from './Codicon.module.css';

// The desktop application draws its icons from the same Codicons package; the page reads the
// package's own SVG sources while it is built, so a drawn icon is the product's icon.
const SOURCES = {
	add,
	'chevron-right': chevronRight,
	close,
	ellipsis,
	'layout-sidebar-left': layoutSidebarLeft,
	'settings-gear': settingsGear,
	terminal,
} as const;

export type CodiconName = keyof typeof SOURCES;

interface IconPath {
	readonly d: string;
	readonly evenOdd: boolean;
}

function attribute(element: string, name: string): string | undefined {
	return new RegExp(`\\s${name}="([^"]*)"`, 'u').exec(element)?.[1];
}

function parse(source: string): { viewBox: string; paths: readonly IconPath[] } {
	const viewBox = attribute(/<svg[^>]*>/u.exec(source)?.[0] ?? '', 'viewBox') ?? '0 0 16 16';
	const paths = [...source.matchAll(/<path[^>]*>/gu)].map(([element]) => ({
		d: attribute(element, 'd') ?? '',
		evenOdd: attribute(element, 'fill-rule') === 'evenodd',
	}));
	return { viewBox, paths };
}

/**
 * A Codicon as the product draws it: 14 px, or 12 px for a disclosure chevron, in the current
 * text color. It is decoration and hidden from assistive technology.
 */
export function Codicon({
	name,
	size = 'regular',
}: {
	name: CodiconName;
	size?: 'regular' | 'disclosure';
}): ReactElement {
	const { viewBox, paths } = parse(SOURCES[name]);
	return (
		<svg
			className={size === 'disclosure' ? styles['disclosure'] : styles['icon']}
			viewBox={viewBox}
			aria-hidden='true'
		>
			{paths.map(({ d, evenOdd }) => (
				<path key={d} d={d} fillRule={evenOdd ? 'evenodd' : undefined} />
			))}
		</svg>
	);
}
