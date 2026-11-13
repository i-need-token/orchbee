const COPIED_FEEDBACK_MS = 1600;

// Adds the behavior the static pages cannot carry: copy buttons, and a language link that keeps
// the visitor's place on the page.
export function activateWebsite(): void {
	for (const link of document.querySelectorAll<HTMLAnchorElement>('[data-language-link]')) {
		link.addEventListener('click', () => {
			link.hash = window.location.hash;
		});
	}
	if (!window.isSecureContext || navigator.clipboard === undefined) return;
	for (const button of document.querySelectorAll<HTMLButtonElement>('[data-copy]')) {
		const label = button.textContent;
		const copiedLabel = button.dataset['copiedLabel'] ?? label;
		let reset: number | undefined;
		button.hidden = false;
		button.addEventListener('click', () => {
			void navigator.clipboard.writeText(button.dataset['copy'] ?? '').then(() => {
				button.dataset['copied'] = '';
				button.textContent = copiedLabel;
				window.clearTimeout(reset);
				reset = window.setTimeout(() => {
					delete button.dataset['copied'];
					button.textContent = label;
				}, COPIED_FEEDBACK_MS);
			});
		});
	}
}
