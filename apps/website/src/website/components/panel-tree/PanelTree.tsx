import type { ComponentType, ReactElement, ReactNode } from 'react';

import type { TerminalRow } from '../terminal-text/terminal-row';
import { panelRows } from './panelRows';
import { isSplit, zoomedLeaf, type PanelLayout, type PanelNode } from './panel-tree';
import styles from './PanelTree.module.css';

/**
 * What a window draws for one Panel. Its root carries `data-leaf` (the Panel's id) and
 * `data-focus` when it has focus; it draws `rows` in a TerminalText bound to `panel`, so script can
 * write them again. `alone` is true while the Panel is the only one, which a window may use to hide
 * the Panel's header; an element marked `data-panel-close` closes the Panel when clicked.
 */
export interface PanelLeafProps {
	readonly id: number;
	readonly focus: boolean;
	readonly alone: boolean;
	readonly rows: readonly TerminalRow[];
}

export type PanelLeafComponent = ComponentType<PanelLeafProps>;

function Split({
	splitKey,
	axis,
	first,
	second,
}: {
	splitKey?: number;
	axis?: 'row' | 'column';
	first?: ReactNode;
	second?: ReactNode;
}): ReactElement {
	return (
		<div className={styles['split']} data-split={splitKey} data-axis={axis}>
			<div className={styles['part']} data-part='first'>
				{first}
			</div>
			<div className={styles['divider']} data-divider='' />
			<div className={styles['part']} data-part='second'>
				{second}
			</div>
		</div>
	);
}

function Node({
	node,
	layout,
	Leaf,
}: {
	node: PanelNode;
	layout: PanelLayout;
	Leaf: PanelLeafComponent;
}): ReactElement {
	if (!isSplit(node)) {
		const focus = node.id === layout.focus;
		return (
			<Leaf
				id={node.id}
				focus={focus}
				alone={!isSplit(layout.tree)}
				rows={panelRows(node.id, focus)}
			/>
		);
	}
	return (
		<Split
			splitKey={node.key}
			axis={node.axis}
			first={<Node node={node.first} layout={layout} Leaf={Leaf} />}
			second={<Node node={node.second} layout={layout} Leaf={Leaf} />}
		/>
	);
}

/**
 * The Panels of a layout as a window draws them: splits drawn here, each Panel by `Leaf`. It fills
 * its parent, a flex container the window sizes. Beside the drawing it renders the parts script
 * clones to draw a changed layout, so every element script adds comes from the page.
 */
export function PanelTree({
	layout,
	Leaf,
}: {
	layout: PanelLayout;
	Leaf: PanelLeafComponent;
}): ReactElement {
	const zoomed = zoomedLeaf(layout);
	return (
		<>
			<div className={styles['slot']} data-panel-slot=''>
				{zoomed === undefined ? (
					<Node node={layout.tree} layout={layout} Leaf={Leaf} />
				) : (
					<Leaf id={zoomed.id} focus alone={false} rows={panelRows(zoomed.id, true)} />
				)}
			</div>
			<template data-panel-parts=''>
				<Split splitKey={0} />
				<Leaf id={0} focus={false} alone={false} rows={[]} />
			</template>
		</>
	);
}
