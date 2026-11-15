import type { ActivationContext } from '../activation';

// A language link leads to the same page in the other language. Without script it opens that page
// at its top; with script it also keeps the section the visitor is reading.
export function activateLanguageLinks({ document }: ActivationContext): void {
	for (const link of document.querySelectorAll<HTMLAnchorElement>('[data-language-link]'))
		link.addEventListener('click', () => {
			link.hash = document.location.hash;
		});
}
