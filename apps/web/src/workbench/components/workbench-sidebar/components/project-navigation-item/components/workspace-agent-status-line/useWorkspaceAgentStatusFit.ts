import { type RefObject, useLayoutEffect, useRef, useState } from 'react';

import {
	fitWorkspaceAgentStatus,
	type WorkspaceAgentStatusFit,
	type WorkspaceAgentStatusMetrics,
} from './fitWorkspaceAgentStatus';

interface WorkspaceAgentStatusFitHandle {
	readonly fit: WorkspaceAgentStatusFit;
	/** The glyph-sized box the fit measures one state glyph with. */
	readonly glyphMeasureRef: RefObject<HTMLSpanElement | null>;
	/** The status line, whose width bounds the glyphs and the remainder. */
	readonly lineRef: RefObject<HTMLSpanElement | null>;
	/** The box the fit measures each `+N` remainder with. */
	readonly remainderMeasureRef: RefObject<HTMLSpanElement | null>;
}

/**
 * Fits a folded Workspace's `total` Agent states to its status line, measured in the layout before
 * paint and again whenever the line's width changes. Until the first measurement, every state shows.
 */
export function useWorkspaceAgentStatusFit(total: number): WorkspaceAgentStatusFitHandle {
	const lineRef = useRef<HTMLSpanElement>(null);
	const glyphMeasureRef = useRef<HTMLSpanElement>(null);
	const remainderMeasureRef = useRef<HTMLSpanElement>(null);
	const [metrics, setMetrics] = useState<WorkspaceAgentStatusMetrics | null>(null);

	useLayoutEffect(() => {
		const line = lineRef.current;
		const glyph = glyphMeasureRef.current;
		const remainder = remainderMeasureRef.current;
		if (line === null || glyph === null || remainder === null) return undefined;
		const measure = () => {
			setMetrics(measureWorkspaceAgentStatus(line, glyph, remainder, total));
		};
		measure();
		if (typeof ResizeObserver === 'undefined') return undefined;
		const observer = new ResizeObserver(measure);
		observer.observe(line);
		return () => {
			observer.disconnect();
		};
	}, [total]);

	return {
		fit: metrics === null ? { hidden: 0, shown: total } : fitWorkspaceAgentStatus(total, metrics),
		glyphMeasureRef,
		lineRef,
		remainderMeasureRef,
	};
}

function measureWorkspaceAgentStatus(
	line: HTMLElement,
	glyph: HTMLElement,
	remainder: HTMLElement,
	total: number,
): WorkspaceAgentStatusMetrics {
	const remainders: number[] = [];
	for (let hidden = 1; hidden <= total; hidden += 1) {
		remainder.textContent = `+${hidden}`;
		remainders.push(
			remainder.getBoundingClientRect().width +
				readLength(getComputedStyle(remainder).marginInlineStart),
		);
	}
	remainder.textContent = '';
	return {
		available: line.getBoundingClientRect().width,
		gap: readLength(getComputedStyle(line).columnGap),
		glyph: glyph.getBoundingClientRect().width,
		remainders,
	};
}

/** A computed length in CSS pixels; a value the environment does not resolve counts as none. */
function readLength(value: string): number {
	const length = Number.parseFloat(value);
	return Number.isFinite(length) ? length : 0;
}
