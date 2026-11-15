import type { ActivationContext } from '../../activation';

// Content marked data-reveal rises into place the first time it scrolls into view. The page
// stylesheet hides it only while the document carries data-motion, which the entry sets before
// the body renders and never under reduced motion.
export function activateReveal({ document, reducedMotion }: ActivationContext): void {
	const targets = document.querySelectorAll<HTMLElement>('[data-reveal]');
	if (reducedMotion || targets.length === 0) return;
	const observer = new IntersectionObserver(
		(entries) => {
			for (const entry of entries) {
				if (!entry.isIntersecting) continue;
				entry.target.setAttribute('data-in', '');
				observer.unobserve(entry.target);
			}
		},
		{ rootMargin: '0px 0px -6% 0px', threshold: 0.06 },
	);
	for (const target of targets) observer.observe(target);
}
