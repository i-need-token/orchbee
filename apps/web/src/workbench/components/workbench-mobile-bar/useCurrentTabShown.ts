import { type RefObject, useLayoutEffect, useRef, useState } from 'react';

interface CurrentTabShownHandle {
	/** The current Tab's place, which clips the second line the Tab wraps to. */
	readonly placeRef: RefObject<HTMLSpanElement | null>;
	readonly shown: boolean;
	readonly tabRef: RefObject<HTMLButtonElement | null>;
}

/**
 * Whether the current Tab stands on its place's first line. A place narrower than the Tab's icon
 * wraps the Tab to a second line, below the place's clip, where nobody can see it; the bar then
 * hides it, so it leaves the tab order and the accessibility tree. Hiding changes no layout, so it
 * cannot move the Tab back. Until the first observation, and without IntersectionObserver, the Tab
 * shows. `present` follows whether the bar renders a current Tab at all.
 */
export function useCurrentTabShown(present: boolean): CurrentTabShownHandle {
	const placeRef = useRef<HTMLSpanElement>(null);
	const tabRef = useRef<HTMLButtonElement>(null);
	const [shown, setShown] = useState(true);

	useLayoutEffect(() => {
		const place = placeRef.current;
		const tab = tabRef.current;
		if (!present || place === null || tab === null) return undefined;
		if (typeof IntersectionObserver === 'undefined') return undefined;
		// The second line starts at the place's bottom edge, which an observer counts as touching
		// it; the margin leaves a Tab there outside.
		const observer = new IntersectionObserver(
			(entries) => {
				const entry = entries.at(-1);
				if (entry !== undefined) setShown(entry.isIntersecting);
			},
			{ root: place, rootMargin: '0px 0px -1px 0px' },
		);
		observer.observe(tab);
		return () => {
			observer.disconnect();
			setShown(true);
		};
	}, [present]);

	return { placeRef, shown, tabRef };
}
