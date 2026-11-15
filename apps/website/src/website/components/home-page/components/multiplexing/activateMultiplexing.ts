import type { ActivationContext } from '../../../../activation';
import { bindScene } from '../../../../scene/bindScene';
import { playScene, type ScenePlayer } from '../../../../scene/playScene';
import {
	MULTIPLEXING_INITIAL,
	createMultiplexingScene,
	multiplexingFrame,
	ran,
	typedAt,
	type MultiplexingState,
} from './multiplexing-scene';

// Restarts the outside console's flash: the attribute comes off, the element reflows, and the
// attribute goes back on.
function flash(element: HTMLElement | null): void {
	if (element === null) return;
	element.removeAttribute('data-flash');
	void element.offsetWidth;
	element.setAttribute('data-flash', '');
}

// Shows the field and makes the shared shell take the visitor's input: what they type shows at
// the prompt in both clients, Enter runs it, and the outside console reads the result. Until the
// visitor focuses the field, the shell runs its sample commands while on screen, never under
// reduced motion. Nothing typed leaves the page.
export function activateMultiplexing({ document, reducedMotion }: ActivationContext): void {
	const root = document.querySelector<HTMLElement>('[data-scene="multiplexing"]');
	const field = root?.querySelector<HTMLElement>('[data-multiplexing-field]');
	const input = field?.querySelector('input');
	if (root === null || root === undefined || field === null || field === undefined) return;
	if (input === null || input === undefined) return;
	const reader = root
		.querySelector('[data-scene-rows="reader"]')
		?.closest<HTMLElement>('[data-terminal]');

	const show = bindScene(root);
	let state: MultiplexingState = MULTIPLEXING_INITIAL;
	show(multiplexingFrame(state));
	const render = (next: MultiplexingState): void => {
		if (next.runs !== state.runs) flash(reader ?? null);
		state = next;
		show(multiplexingFrame(next));
	};

	let player: ScenePlayer<MultiplexingState> | undefined = reducedMotion
		? undefined
		: playScene(createMultiplexingScene(), {
				root,
				watch: [...root.querySelectorAll('[data-window-fit]')],
				render,
			});
	// The visitor takes over: the samples stop for good and the prompt empties.
	const takeOver = (): void => {
		if (player === undefined) return;
		player.stop();
		player = undefined;
		render(typedAt(state, ''));
	};

	input.addEventListener('focus', takeOver);
	input.addEventListener('input', () => {
		takeOver();
		render(typedAt(state, input.value));
	});
	input.addEventListener('keydown', (event) => {
		if (event.key !== 'Enter' || event.isComposing) return;
		takeOver();
		render(ran(state, input.value));
		input.value = '';
	});
	field.hidden = false;
}
