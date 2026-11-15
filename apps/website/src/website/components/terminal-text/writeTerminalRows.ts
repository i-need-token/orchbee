import type { TerminalRow, TerminalSegment } from './terminal-row';

// Writes rows into the rows element of a prerendered TerminalText, the way the component renders
// them. Elements that carry a class are cloned from the terminal's template; script only adds the
// unstyled runs inside a row (`b`, and `span` with a `data-tone`).

function part(template: HTMLTemplateElement, name: 'row' | 'cursor'): Element | undefined {
	const element = template.content.querySelector(`[data-part="${name}"]`)?.cloneNode(false);
	if (!(element instanceof Element)) return undefined;
	element.removeAttribute('data-part');
	return element;
}

function segmentNode(
	document: Document,
	template: HTMLTemplateElement,
	segment: TerminalSegment,
): Node | undefined {
	if (typeof segment === 'string') return document.createTextNode(segment);
	if ('cursor' in segment) {
		const cursor = part(template, 'cursor');
		cursor?.toggleAttribute('data-blink', segment.cursor === 'blink');
		return cursor;
	}
	const run = document.createElement(segment.tone === 'bold' ? 'b' : 'span');
	if (segment.tone !== 'bold') run.dataset['tone'] = segment.tone;
	run.textContent = segment.text;
	return run;
}

/** Replaces the rows shown in `rows`, an element marked `data-scene-rows` inside a TerminalText. */
export function writeTerminalRows(rows: Element, content: readonly TerminalRow[]): void {
	const template = rows.closest('[data-terminal]')?.querySelector(':scope > template');
	if (!(template instanceof HTMLTemplateElement)) return;
	const document = rows.ownerDocument;
	const elements: Node[] = [];
	for (const terminalRow of content) {
		const element = part(template, 'row');
		if (element === undefined) return;
		element.toggleAttribute('data-input', terminalRow.input === true);
		for (const segment of terminalRow.segments) {
			const node = segmentNode(document, template, segment);
			if (node !== undefined) element.append(node);
		}
		elements.push(element);
	}
	rows.replaceChildren(...elements);
}
