// Shows the OptionSwitch named `name` inside `root` and calls `pick` with the chosen option's id
// and its button each time the visitor presses one; the pressed button follows the choice.
export function bindOptionSwitch(
	root: Element,
	name: string,
	pick: (option: string, button: HTMLButtonElement) => void,
): void {
	const row = root.querySelector<HTMLElement>(`[data-option-switch="${name}"]`);
	if (row === null) return;
	const buttons = [...row.querySelectorAll<HTMLButtonElement>('button[data-option]')];
	for (const button of buttons)
		button.addEventListener('click', () => {
			const option = button.dataset['option'] ?? '';
			for (const other of buttons) other.setAttribute('aria-pressed', String(other === button));
			pick(option, button);
		});
	row.hidden = false;
}
