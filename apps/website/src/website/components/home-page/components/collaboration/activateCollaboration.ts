import type { ActivationContext } from '../../../../activation';
import { bindScene } from '../../../../scene/bindScene';
import { playScene } from '../../../../scene/playScene';
import { sceneFinalState } from '../../../../scene/scene';
import {
	collaborationAfter,
	collaborationFrame,
	createCollaborationScene,
	stepAt,
	stepDuration,
	stepProgress,
	stepStart,
	type CollaborationState,
} from './collaboration-scene';

/** The share of the figure that must show for it to play. */
const VISIBLE_SHARE = 0.2;

// Plays the five steps in turn while the figure is on screen, never under reduced motion, and
// makes the rail's steps buttons: choosing one replays the figure from that step, or, when nothing
// plays, shows the figure once that step has run. Off screen the figure stops, and it starts the
// step it was on again when it comes back.
export function activateCollaboration({ document, reducedMotion }: ActivationContext): void {
	const root = document.querySelector<HTMLElement>('[data-scene="collaboration"]');
	if (root === null) return;
	const steps = [...root.querySelectorAll<HTMLElement>('[data-step]')];
	const scene = createCollaborationScene();
	const show = bindScene(root);
	show(collaborationFrame(sceneFinalState(scene)));

	// The rail follows the step under way. A line changes at once, except the line of the step
	// under way, which fills over the step's length: it empties, reflows, takes the step's length
	// as its transition time, and fills, so a step that starts again restarts its line.
	let marked = '';
	const mark = (state: CollaborationState): void => {
		const key = `${state.step} ${state.running}`;
		if (key === marked) return;
		marked = key;
		steps.forEach((step, index) => {
			const line = step.querySelector<HTMLElement>('[data-step-progress]');
			const progress = stepProgress(state, index);
			line?.style.removeProperty('transition-duration');
			step.removeAttribute('data-on');
			step.toggleAttribute('data-done', progress === 'done');
			if (progress !== 'on' || line === null) return;
			void line.offsetWidth;
			line.style.transitionDuration = `${stepDuration(index)}ms`;
			step.setAttribute('data-on', '');
		});
	};
	const render = (state: CollaborationState): void => {
		show(collaborationFrame(state));
		mark(state);
	};

	const choose = reducedMotion
		? (index: number): void => {
				render(collaborationAfter(index));
			}
		: (() => {
				render(scene.initial);
				const player = playScene(scene, {
					root,
					watch: [root],
					threshold: VISIBLE_SHARE,
					resumeAt: (paused) => {
						marked = '';
						return stepStart(stepAt(paused));
					},
					render,
				});
				return (index: number): void => {
					marked = '';
					player.seek(stepStart(index));
				};
			})();

	steps.forEach((step, index) => {
		const button = step.querySelector('button');
		if (button === null) return;
		button.addEventListener('click', () => {
			choose(index);
		});
		button.disabled = false;
	});
}
