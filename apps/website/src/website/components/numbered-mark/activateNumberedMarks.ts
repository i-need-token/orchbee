import type { ActivationContext } from '../../activation';

// Lights every element of a `data-marks` scope that names the same mark while one of them is under
// the pointer or a note has keyboard focus. Notes become focusable here, because focusing them
// only means something once script lights their parts.
export function activateNumberedMarks({ document }: ActivationContext): void {
	for (const scope of document.querySelectorAll<HTMLElement>('[data-marks]')) {
		const targets = [...scope.querySelectorAll<HTMLElement>('[data-mark]')];
		let hovered: string | undefined;
		let focused: string | undefined;
		const show = (): void => {
			for (const target of targets) {
				const mark = target.dataset['mark'];
				target.toggleAttribute('data-hot', mark === hovered || mark === focused);
			}
		};
		for (const target of targets) {
			const mark = target.dataset['mark'];
			target.addEventListener('pointerenter', () => {
				hovered = mark;
				show();
			});
			target.addEventListener('pointerleave', () => {
				if (hovered === mark) hovered = undefined;
				show();
			});
			if (!target.hasAttribute('data-mark-note')) continue;
			target.tabIndex = 0;
			target.addEventListener('focus', () => {
				focused = mark;
				show();
			});
			target.addEventListener('blur', () => {
				if (focused === mark) focused = undefined;
				show();
			});
		}
	}
}
