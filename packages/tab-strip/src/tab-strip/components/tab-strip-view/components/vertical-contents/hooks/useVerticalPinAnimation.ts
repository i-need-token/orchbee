import { useLayoutEffect, useRef, useState } from 'react';
import type { TabModelSnapshot } from '../../../../../model/tab-model';
import {
	TAB_BOUNDS_ANIMATION_DURATION_MS,
	cancelManagedAnimationFrame,
	requestManagedAnimationFrame,
} from '../../../../../utils/tab-strip-internals';
import { VerticalPinAnimationRuntime } from '../model/VerticalPinAnimationRuntime';
import {
	buildVerticalPinAnimationEntries,
	type VerticalPinAnimationEntries,
} from '../utils/buildVerticalPinAnimationEntries';

export function useVerticalPinAnimation({
	snapshot,
}: Readonly<{
	snapshot: TabModelSnapshot;
}>): VerticalPinAnimationEntries {
	const runtimeRef = useRef<VerticalPinAnimationRuntime | null>(null);
	if (runtimeRef.current === null) runtimeRef.current = new VerticalPinAnimationRuntime();
	const animationRequestRef = useRef<number | null>(null);
	const [animations, setAnimations] = useState(runtimeRef.current.currentAnimations);

	useLayoutEffect(() => {
		const runtime = runtimeRef.current;
		if (!runtime?.sync(snapshot, performance.now())) return;
		if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) {
			runtime.complete();
			setAnimations([]);
			return;
		}
		setAnimations(runtime.currentAnimations);
		if (animationRequestRef.current !== null) return;

		const tick = (now: number) => {
			const next = runtime.advance(now, TAB_BOUNDS_ANIMATION_DURATION_MS);
			setAnimations(next);
			if (runtime.isAnimating) {
				animationRequestRef.current = requestManagedAnimationFrame(tick);
			} else {
				animationRequestRef.current = null;
			}
		};
		animationRequestRef.current = requestManagedAnimationFrame(tick);
	}, [snapshot]);

	useLayoutEffect(
		() => () => {
			if (animationRequestRef.current !== null) {
				cancelManagedAnimationFrame(animationRequestRef.current);
				animationRequestRef.current = null;
			}
			runtimeRef.current?.complete();
		},
		[],
	);

	return buildVerticalPinAnimationEntries({
		snapshot,
		animations,
	});
}
