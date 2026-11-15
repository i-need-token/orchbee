import { writeTerminalRows } from '../terminal-text/writeTerminalRows';
import { panelRows } from './panelRows';
import {
	isSplit,
	zoomedLeaf,
	type PanelBox,
	type PanelLayout,
	type PanelNode,
	type PanelSize,
	type PanelSplit,
} from './panel-tree';

// Draws a layout in a window that a PanelTree rendered: script clones the split and Panel parts the
// page carries beside the slot, so it never builds a classed element itself, and writes split
// ratios through the CSSOM.

/** How the window shows a layout and what the model needs to measure in it. */
export interface PanelView {
	/** Draws the whole layout again; `fresh` names a Panel that has just appeared. */
	draw(layout: PanelLayout, fresh?: number): void;
	/** Shows a new focus in the drawn Panels. */
	showFocus(layout: PanelLayout): void;
	/** Shows a split's new ratio. */
	showRatio(layout: PanelLayout, key: number): void;
	/** A Panel's size in the window's own pixels, at any zoom. */
	room(id: number): PanelSize | undefined;
	/** Where every drawn Panel is on the page. */
	boxes(): PanelBox[];
}

function partOf(parts: HTMLTemplateElement, selector: string): Element | undefined {
	const part = parts.content.querySelector(selector)?.cloneNode(true);
	return part instanceof Element ? part : undefined;
}

function applyRatio(split: Element, ratio: number): void {
	const first = split.querySelector(':scope > [data-part="first"]');
	const second = split.querySelector(':scope > [data-part="second"]');
	if (first instanceof HTMLElement) first.style.flexGrow = String(ratio);
	if (second instanceof HTMLElement) second.style.flexGrow = String(1 - ratio);
}

function findSplit(node: PanelNode, key: number): PanelSplit | undefined {
	if (!isSplit(node)) return undefined;
	if (node.key === key) return node;
	return findSplit(node.first, key) ?? findSplit(node.second, key);
}

function showLeaf(leaf: Element, layout: PanelLayout): void {
	const id = Number(leaf.getAttribute('data-leaf'));
	const focused = id === layout.focus;
	leaf.toggleAttribute('data-focus', focused);
	const rows = leaf.querySelector('[data-scene-rows]');
	if (rows !== null) writeTerminalRows(rows, panelRows(id, focused));
}

/**
 * The view of the window whose PanelTree is inside `window`, or undefined when the window lacks
 * one. `window` is the drawn window's root, whose own width is its unzoomed pixel width.
 */
export function createPanelView(window: HTMLElement): PanelView | undefined {
	const slot = window.querySelector('[data-panel-slot]');
	const parts = window.querySelector('template[data-panel-parts]');
	if (!(slot instanceof HTMLElement) || !(parts instanceof HTMLTemplateElement)) return undefined;
	const drawnLeaves = (): HTMLElement[] => [...slot.querySelectorAll<HTMLElement>('[data-leaf]')];

	const leafElement = (id: number, layout: PanelLayout, alone: boolean, fresh?: number) => {
		const leaf = partOf(parts, '[data-leaf]');
		if (leaf === undefined) return undefined;
		leaf.setAttribute('data-leaf', String(id));
		leaf.toggleAttribute('data-alone', alone);
		leaf.toggleAttribute('data-fresh', id === fresh);
		showLeaf(leaf, layout);
		return leaf;
	};
	const nodeElement = (
		node: PanelNode,
		layout: PanelLayout,
		fresh?: number,
	): Element | undefined => {
		if (!isSplit(node)) return leafElement(node.id, layout, !isSplit(layout.tree), fresh);
		const split = partOf(parts, '[data-split]');
		if (split === undefined) return undefined;
		split.setAttribute('data-split', String(node.key));
		split.setAttribute('data-axis', node.axis);
		const first = nodeElement(node.first, layout, fresh);
		const second = nodeElement(node.second, layout, fresh);
		if (first !== undefined) split.querySelector(':scope > [data-part="first"]')?.append(first);
		if (second !== undefined) split.querySelector(':scope > [data-part="second"]')?.append(second);
		applyRatio(split, node.ratio);
		return split;
	};

	return {
		draw(layout, fresh) {
			const zoomed = zoomedLeaf(layout);
			const drawn =
				zoomed === undefined
					? nodeElement(layout.tree, layout, fresh)
					: leafElement(zoomed.id, layout, false, fresh);
			if (drawn !== undefined) slot.replaceChildren(drawn);
		},
		showFocus(layout) {
			for (const leaf of drawnLeaves()) showLeaf(leaf, layout);
		},
		showRatio(layout, key) {
			const split = findSplit(layout.tree, key);
			const element = slot.querySelector(`[data-split="${key}"]`);
			if (split !== undefined && element !== null) applyRatio(element, split.ratio);
		},
		room(id) {
			const leaf = drawnLeaves().find((element) => Number(element.dataset['leaf']) === id);
			if (leaf === undefined) return undefined;
			const scale = window.getBoundingClientRect().width / window.offsetWidth || 1;
			const box = leaf.getBoundingClientRect();
			return { width: box.width / scale, height: box.height / scale };
		},
		boxes() {
			return drawnLeaves().map((leaf) => {
				const { left, top, right, bottom } = leaf.getBoundingClientRect();
				return { id: Number(leaf.dataset['leaf']), left, top, right, bottom };
			});
		},
	};
}
