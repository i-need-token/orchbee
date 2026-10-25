import type { ProposedLayout } from './proposed-layout';

export interface AnimatedLayoutFrame {
	layout: ProposedLayout;
	contentHeight: number;
}

function clamp01(value: number): number {
	return Math.min(1, Math.max(0, value));
}

export function tabCollectionEaseOut(value: number): number {
	const clamped = clamp01(value);
	return 1 - (1 - clamped) ** 2;
}

export function tabCollectionEaseInOut(value: number): number {
	const clamped = clamp01(value);
	return clamped < 0.5 ? 2 * clamped * clamped : 1 - (-2 * clamped + 2) ** 2 / 2;
}
