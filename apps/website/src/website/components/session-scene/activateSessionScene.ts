import type { ActivationContext } from '../../activation';
import { bindScene } from '../../scene/bindScene';
import { playScene } from '../../scene/playScene';
import { createSessionScene, sessionFrame, sessionStrings } from './session';

function pageStrings(root: Element): Map<string, string> {
	const words = new Map<string, string>();
	const template = root.querySelector(':scope > template[data-scene-strings]');
	if (!(template instanceof HTMLTemplateElement)) return words;
	// Parsed HTML puts the words in the template's content; a client render appends them to the
	// template element itself.
	const source = template.content.childElementCount > 0 ? template.content : template;
	for (const word of source.querySelectorAll('[data-key]'))
		words.set(word.getAttribute('data-key') ?? '', word.textContent ?? '');
	return words;
}

// Replays the scripted Session in every SessionScene on the page, while one of its windows is on
// screen. Under reduced motion the page keeps the finished Session it was rendered with.
export function activateSessionScene({ document, reducedMotion }: ActivationContext): void {
	if (reducedMotion) return;
	for (const root of document.querySelectorAll<HTMLElement>('[data-scene="session"]')) {
		const words = pageStrings(root);
		const strings = sessionStrings((key) => words.get(key));
		const show = bindScene(root);
		const windows = [...root.querySelectorAll('[data-window-fit]')];
		playScene(createSessionScene(), {
			root,
			watch: windows.length > 0 ? windows : [root],
			render: (state) => {
				show(sessionFrame(state, strings));
			},
		});
	}
}
