import type { ActivationContext } from '../../../../activation';

// Turns the list of commands into tabs, one per drawn output: one Tab stop for the list, arrow
// keys (either axis, since the list runs down on wide screens and across on narrow ones), Home
// and End move and choose at once, and Tab goes on to the copy button and the output. Choosing a
// command shows its output from the top. Nothing moves by itself.
export function activateCliOutputs({ document }: ActivationContext): void {
	for (const root of document.querySelectorAll<HTMLElement>('[data-cli-outputs]')) {
		const list = root.querySelector<HTMLElement>('[data-output-list]');
		const tabs = [...root.querySelectorAll<HTMLElement>('[data-output-tab]')];
		const panels = [...root.querySelectorAll<HTMLElement>('[data-output-panel]')];
		if (list === null || tabs.length === 0 || tabs.length !== panels.length) continue;

		list.setAttribute('role', 'tablist');
		list.setAttribute('aria-orientation', 'vertical');
		tabs.forEach((tab, index) => {
			const panel = panels[index];
			if (panel === undefined) return;
			tab.setAttribute('role', 'tab');
			tab.setAttribute('aria-controls', panel.id);
			panel.setAttribute('role', 'tabpanel');
			panel.setAttribute('aria-labelledby', tab.id);
		});

		let current = Math.max(
			0,
			tabs.findIndex((tab) => tab.hasAttribute('data-current')),
		);
		const show = (chosen: number): void => {
			current = chosen;
			tabs.forEach((tab, index) => {
				const on = index === chosen;
				tab.toggleAttribute('data-current', on);
				tab.setAttribute('aria-selected', String(on));
				tab.tabIndex = on ? 0 : -1;
			});
			panels.forEach((panel, index) => {
				panel.hidden = index !== chosen;
			});
			const body = panels[chosen]?.querySelector('[data-output-body]');
			if (body !== null && body !== undefined) body.scrollTop = 0;
		};
		show(current);

		tabs.forEach((tab, index) => {
			tab.addEventListener('click', () => {
				if (index !== current) show(index);
			});
		});
		list.addEventListener('keydown', (event) => {
			const last = tabs.length - 1;
			let next: number;
			switch (event.key) {
				case 'ArrowDown':
				case 'ArrowRight':
					next = current === last ? 0 : current + 1;
					break;
				case 'ArrowUp':
				case 'ArrowLeft':
					next = current === 0 ? last : current - 1;
					break;
				case 'Home':
					next = 0;
					break;
				case 'End':
					next = last;
					break;
				default:
					return;
			}
			event.preventDefault();
			show(next);
			tabs[next]?.focus();
		});
	}
}
