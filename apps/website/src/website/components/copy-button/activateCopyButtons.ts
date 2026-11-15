import type { ActivationContext } from '../../activation';

const COPIED_FEEDBACK_MS = 1200;

// Shows every copy button once the page may write to the clipboard, which needs a secure context.
export function activateCopyButtons({ document }: ActivationContext): void {
	const view = document.defaultView;
	if (view === null || !view.isSecureContext || !('clipboard' in view.navigator)) return;
	const { clipboard } = view.navigator;
	for (const button of document.querySelectorAll<HTMLButtonElement>('[data-copy]')) {
		let reset = 0;
		button.hidden = false;
		const copied = (): void => {
			button.dataset['copied'] = '';
			view.clearTimeout(reset);
			reset = view.setTimeout(() => {
				delete button.dataset['copied'];
			}, COPIED_FEEDBACK_MS);
		};
		// A refused write leaves the command on the page to select by hand.
		const refused = (): void => {
			delete button.dataset['copied'];
		};
		button.addEventListener('click', () => {
			void clipboard.writeText(button.dataset['copy'] ?? '').then(copied, refused);
		});
	}
}
