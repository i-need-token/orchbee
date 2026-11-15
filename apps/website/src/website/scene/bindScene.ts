import { writeTerminalRows } from '../components/terminal-text/writeTerminalRows';
import type { SceneFrame } from './scene';

type Bindings = ReadonlyMap<string, readonly Element[]>;

function collect(root: ParentNode, attribute: string): Bindings {
	const bindings = new Map<string, Element[]>();
	for (const element of root.querySelectorAll(`[${attribute}]`)) {
		const key = element.getAttribute(attribute) ?? '';
		const elements = bindings.get(key);
		if (elements === undefined) bindings.set(key, [element]);
		else elements.push(element);
	}
	return bindings;
}

function update<Value>(
	bindings: Bindings,
	values: Readonly<Record<string, Value>>,
	previous: Readonly<Record<string, Value>> | undefined,
	write: (element: Element, value: Value) => void,
): void {
	for (const [key, elements] of bindings) {
		if (!Object.hasOwn(values, key)) continue;
		const value = values[key];
		if (value === undefined || (previous !== undefined && previous[key] === value)) continue;
		for (const element of elements) write(element, value);
	}
}

/**
 * Finds the elements under `root` that name a scene binding key (see SceneFrame) and returns a
 * function that shows a frame in them. Only values that changed since the last frame are
 * written; rows compare by identity, so a scene keeps unchanged rows as the same array.
 */
export function bindScene(root: ParentNode): (frame: SceneFrame) => void {
	const rows = collect(root, 'data-scene-rows');
	const states = collect(root, 'data-scene-state');
	const texts = collect(root, 'data-scene-text');
	const shows = collect(root, 'data-scene-show');
	const opens = collect(root, 'data-scene-open');
	let last: SceneFrame | undefined;
	return (frame) => {
		update(rows, frame.rows, last?.rows, writeTerminalRows);
		update(states, frame.states, last?.states, (element, state) => {
			element.setAttribute('data-state', state);
		});
		update(texts, frame.texts, last?.texts, (element, text) => {
			element.textContent = text;
		});
		update(shows, frame.flags, last?.flags, (element, on) => {
			element.toggleAttribute('hidden', !on);
		});
		update(opens, frame.flags, last?.flags, (element, on) => {
			element.toggleAttribute('data-open', on);
		});
		last = frame;
	};
}
