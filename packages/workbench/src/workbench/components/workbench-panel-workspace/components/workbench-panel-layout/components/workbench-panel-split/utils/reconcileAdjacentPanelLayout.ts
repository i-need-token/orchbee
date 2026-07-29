import { matchesPanelLayoutTopology } from './matchesPanelLayoutTopology';
import { projectAdjacentPanelLayout } from './projectAdjacentPanelLayout';

const PANEL_LAYOUT_TOLERANCE = 0.000_001;

export interface ReconcileAdjacentPanelLayoutInput {
	readonly availablePanelSizePx: number;
	readonly candidateLayout: Readonly<Record<string, number>>;
	readonly childIds: readonly string[];
	readonly minimumPanelSizePx: number;
	readonly modelLayout: Readonly<Record<string, number>>;
}

type AdjacentPanelLayoutRejection =
	| 'invalid-candidate-layout'
	| 'invalid-child-ids'
	| 'invalid-geometry'
	| 'invalid-model-layout'
	| 'non-adjacent-change';

export type AdjacentPanelLayoutReconciliation =
	| {
			readonly kind: 'accepted';
			readonly candidateNeedsCorrection: boolean;
			readonly layout: Readonly<Record<string, number>>;
			readonly leadingWeight: number;
			readonly modelNeedsUpdate: boolean;
			readonly separatorIndex: number;
	  }
	| {
			readonly kind: 'rejected';
			readonly reason: AdjacentPanelLayoutRejection;
	  }
	| {
			readonly kind: 'unchanged';
	  };

export function reconcileAdjacentPanelLayout({
	availablePanelSizePx,
	candidateLayout,
	childIds,
	minimumPanelSizePx,
	modelLayout,
}: ReconcileAdjacentPanelLayoutInput): AdjacentPanelLayoutReconciliation {
	if (new Set(childIds).size !== childIds.length) {
		return { kind: 'rejected', reason: 'invalid-child-ids' };
	}
	if (!matchesPanelLayoutTopology(modelLayout, childIds)) {
		return { kind: 'rejected', reason: 'invalid-model-layout' };
	}
	if (!matchesPanelLayoutTopology(candidateLayout, childIds)) {
		return { kind: 'rejected', reason: 'invalid-candidate-layout' };
	}
	if (
		!Number.isFinite(availablePanelSizePx) ||
		availablePanelSizePx <= 0 ||
		!Number.isFinite(minimumPanelSizePx) ||
		minimumPanelSizePx < 0
	) {
		return { kind: 'rejected', reason: 'invalid-geometry' };
	}

	const changedIndices = childIds.flatMap((childId, index) =>
		Math.abs((candidateLayout[childId] ?? 0) - (modelLayout[childId] ?? 0)) > PANEL_LAYOUT_TOLERANCE
			? [index]
			: [],
	);
	if (changedIndices.length === 0) {
		return { kind: 'unchanged' };
	}
	const leadingIndex = changedIndices[0];
	const trailingIndex = changedIndices[1];
	if (
		changedIndices.length !== 2 ||
		leadingIndex === undefined ||
		trailingIndex !== leadingIndex + 1
	) {
		return { kind: 'rejected', reason: 'non-adjacent-change' };
	}

	const minimumChildWeight = (minimumPanelSizePx / availablePanelSizePx) * 100;
	const projection = projectAdjacentPanelLayout(
		childIds,
		leadingIndex,
		modelLayout,
		candidateLayout,
		minimumChildWeight,
	);
	if (projection === null) {
		return { kind: 'rejected', reason: 'invalid-candidate-layout' };
	}

	return {
		kind: 'accepted',
		candidateNeedsCorrection: childIds.some(
			(childId) =>
				Math.abs((candidateLayout[childId] ?? 0) - (projection.layout[childId] ?? 0)) >
				PANEL_LAYOUT_TOLERANCE,
		),
		layout: projection.layout,
		leadingWeight: projection.leadingWeight,
		modelNeedsUpdate: childIds.some(
			(childId) =>
				Math.abs((modelLayout[childId] ?? 0) - (projection.layout[childId] ?? 0)) >
				PANEL_LAYOUT_TOLERANCE,
		),
		separatorIndex: leadingIndex,
	};
}
