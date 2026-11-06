import { describe, expect, it } from 'vitest';

import {
	fitWorkspaceAgentStatus,
	type WorkspaceAgentStatusMetrics,
} from '../fitWorkspaceAgentStatus';

/** Glyph sizes, gaps and `+N` margins, so the fit is checked over more than one geometry. */
const GEOMETRIES = [
	{ gap: 4, glyph: 12, margin: 4 },
	{ gap: 0, glyph: 7, margin: 0 },
	{ gap: 2, glyph: 10, margin: 2 },
] as const;

const TOTALS = [0, 1, 2, 3, 5, 8, 13, 20] as const;

const WIDEST = 260;

type Geometry = (typeof GEOMETRIES)[number];

/** The `+N` widths for every hidden count up to `total`: a margin, a sign and one cell per digit. */
function remaindersUpTo(total: number, margin: number): number[] {
	return Array.from({ length: total }, (_, index) => margin + 6 + 6 * String(index + 1).length);
}

function metricsFor(
	total: number,
	geometry: Geometry,
	available: number,
): WorkspaceAgentStatusMetrics {
	return {
		available,
		gap: geometry.gap,
		glyph: geometry.glyph,
		remainders: remaindersUpTo(total, geometry.margin),
	};
}

/**
 * The width the line occupies when it shows `shown` glyphs and a `+N` for `hidden`: the glyphs in
 * one run, then the `+N` after one gap. A piece the line does not show takes no room.
 */
function occupiedWidth(
	metrics: WorkspaceAgentStatusMetrics,
	shown: number,
	hidden: number,
): number {
	const glyphs = shown === 0 ? 0 : shown * metrics.glyph + (shown - 1) * metrics.gap;
	const remainder = hidden === 0 ? 0 : (metrics.remainders[hidden - 1] ?? Number.POSITIVE_INFINITY);
	const pieces = Number(shown > 0) + Number(hidden > 0);
	return glyphs + remainder + Math.max(pieces - 1, 0) * metrics.gap;
}

/** How a fit breaks the rule at one width, as descriptions; none when it obeys the rule. */
function ruleViolations(total: number, geometry: Geometry, available: number): string[] {
	const metrics = metricsFor(total, geometry, available);
	const fit = fitWorkspaceAgentStatus(total, metrics);
	const nothingFits = fit.shown === 0 && fit.hidden === 0 && total > 0;
	const violations: string[] = [];
	const reportedWidth = occupiedWidth(metrics, fit.shown, fit.hidden);
	if (!nothingFits && (fit.shown + fit.hidden !== total || reportedWidth > available)) {
		violations.push(`reports ${fit.shown} glyphs and +${fit.hidden}`);
	}
	for (let shown = fit.shown + 1; shown <= total; shown += 1) {
		if (occupiedWidth(metrics, shown, total - shown) <= available) {
			violations.push(`${shown} glyphs and +${total - shown} fit but are not shown`);
		}
	}
	if (nothingFits && occupiedWidth(metrics, 0, total) <= available) {
		violations.push(`+${total} fits but nothing is shown`);
	}
	return violations;
}

describe('fitWorkspaceAgentStatus', () => {
	it('shows every state when their glyphs fit the line', () => {
		// Five glyphs and four gaps take 5 * 12 + 4 * 4 = 76 cells.
		const metrics = metricsFor(5, GEOMETRIES[0], 76);
		expect(fitWorkspaceAgentStatus(5, metrics)).toEqual({ hidden: 0, shown: 5 });
		expect(fitWorkspaceAgentStatus(5, { ...metrics, available: 75 }).hidden).toBeGreaterThan(0);
	});

	it('shows the most glyphs that fit beside their +N', () => {
		// Three glyphs and +7 take 44 + 4 + 16 = 64 cells; four glyphs and +6 take 60 + 4 + 16 = 80.
		const metrics = metricsFor(10, GEOMETRIES[0], 70);
		expect(fitWorkspaceAgentStatus(10, metrics)).toEqual({ hidden: 7, shown: 3 });
	});

	it('shows nothing when not even a +N fits', () => {
		expect(fitWorkspaceAgentStatus(3, metricsFor(3, GEOMETRIES[0], 10))).toEqual({
			hidden: 0,
			shown: 0,
		});
	});

	it('never shows a remainder it has not measured, so the line does not overflow', () => {
		const metrics: WorkspaceAgentStatusMetrics = {
			available: 20,
			gap: 4,
			glyph: 12,
			remainders: [],
		};
		expect(fitWorkspaceAgentStatus(3, metrics)).toEqual({ hidden: 0, shown: 0 });
	});

	it('reports a layout that fits the line, and no larger count of glyphs that would', () => {
		const violations: string[] = [];
		for (const geometry of GEOMETRIES) {
			for (const total of TOTALS) {
				for (let available = 0; available <= WIDEST; available += 3) {
					for (const violation of ruleViolations(total, geometry, available)) {
						violations.push(
							`glyph ${geometry.glyph}, gap ${geometry.gap}, ${total} states, line ${available}: ${violation}`,
						);
					}
				}
			}
		}
		expect(violations).toEqual([]);
	});

	it('counts a layout exactly as wide as the line as fitting', () => {
		for (const geometry of GEOMETRIES) {
			for (const total of TOTALS) {
				for (let shown = 0; shown <= total; shown += 1) {
					const metrics = metricsFor(total, geometry, 0);
					const available = occupiedWidth(metrics, shown, total - shown);
					const fit = fitWorkspaceAgentStatus(total, { ...metrics, available });
					expect(fit.shown).toBeGreaterThanOrEqual(shown);
				}
			}
		}
	});
});
