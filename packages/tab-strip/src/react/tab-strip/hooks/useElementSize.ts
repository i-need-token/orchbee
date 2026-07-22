import { useEffect, useRef, useState, type RefObject } from 'react';

export function useElementSize<T extends HTMLElement>(): readonly [
	RefObject<T | null>,
	Readonly<{ width: number; height: number }>,
] {
	const ref = useRef<T>(null);
	const [size, setSize] = useState({ width: 0, height: 0 });
	useEffect(() => {
		const element = ref.current;
		if (!element) return;
		const update = () => {
			const rect = element.getBoundingClientRect();
			setSize({ width: Math.round(rect.width), height: Math.round(rect.height) });
		};
		update();
		const observer = new ResizeObserver(update);
		observer.observe(element);
		return () => observer.disconnect();
	}, []);
	return [ref, size];
}
