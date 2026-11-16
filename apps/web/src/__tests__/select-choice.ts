import { within } from '@testing-library/react';
import { act } from 'react';

/** The shared select's trigger named `label` inside `container`, as a person finds it. */
export function selectTrigger(container: HTMLElement, label: string): HTMLButtonElement {
	return within(container).getByRole<HTMLButtonElement>('combobox', { name: label });
}

/** The entry `trigger` shows. */
export function selectedText(trigger: HTMLElement): string {
	return trigger.querySelector('[data-slot="select-value"]')?.textContent ?? '';
}

/** Opens `trigger`'s list and clicks the entry named `option`, as a mouse would. */
export async function chooseSelectOption(trigger: HTMLElement, option: string): Promise<void> {
	await act(async () => {
		trigger.dispatchEvent(new MouseEvent('click', { bubbles: true }));
	});
	const entry = within(trigger.ownerDocument.body).getByRole('option', { name: option });
	await act(async () => {
		entry.dispatchEvent(new MouseEvent('pointerdown', { bubbles: true }));
		entry.dispatchEvent(new MouseEvent('click', { bubbles: true }));
	});
}
