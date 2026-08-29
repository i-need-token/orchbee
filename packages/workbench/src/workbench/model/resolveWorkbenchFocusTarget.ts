const WORKBENCH_FOCUS_REGIONS = ['terminal', 'tab-strip', 'sidebar'] as const;

export type WorkbenchFocusRegion = (typeof WORKBENCH_FOCUS_REGIONS)[number];

export interface WorkbenchTerminalFocusRequest {
	readonly presentationId: string;
	readonly sequence: number;
}

export type WorkbenchFocusIntent =
	| { readonly direction: 'next' | 'previous'; readonly kind: 'cycle' }
	| { readonly kind: 'direct'; readonly region: WorkbenchFocusRegion };

export type WorkbenchFocusResolution =
	| { readonly kind: 'target'; readonly region: WorkbenchFocusRegion }
	| { readonly kind: 'unchanged'; readonly region: WorkbenchFocusRegion }
	| {
			readonly kind: 'unavailable';
			readonly reason:
				| 'exclusive-surface-active'
				| 'no-available-region'
				| 'no-different-region'
				| 'target-unavailable';
	  };

export function resolveWorkbenchFocusTarget({
	availableRegions,
	currentRegion,
	exclusiveSurfaceActive,
	intent,
}: {
	readonly availableRegions: readonly WorkbenchFocusRegion[];
	readonly currentRegion: WorkbenchFocusRegion | null;
	readonly exclusiveSurfaceActive: boolean;
	readonly intent: WorkbenchFocusIntent;
}): WorkbenchFocusResolution {
	if (exclusiveSurfaceActive) {
		return { kind: 'unavailable', reason: 'exclusive-surface-active' };
	}

	const available = WORKBENCH_FOCUS_REGIONS.filter((region) => availableRegions.includes(region));
	if (available.length === 0) return { kind: 'unavailable', reason: 'no-available-region' };

	if (intent.kind === 'direct') {
		if (!available.includes(intent.region)) {
			return { kind: 'unavailable', reason: 'target-unavailable' };
		}
		return currentRegion === intent.region
			? { kind: 'unchanged', region: intent.region }
			: { kind: 'target', region: intent.region };
	}

	const currentIndex = currentRegion === null ? -1 : available.indexOf(currentRegion);
	const firstAvailable = available[0];
	if (firstAvailable === undefined) {
		return { kind: 'unavailable', reason: 'no-available-region' };
	}
	if (currentIndex === -1) return { kind: 'target', region: firstAvailable };
	if (available.length === 1) return { kind: 'unavailable', reason: 'no-different-region' };

	const offset = intent.direction === 'next' ? 1 : -1;
	const targetIndex = (currentIndex + offset + available.length) % available.length;
	const target = available[targetIndex];
	return target === undefined
		? { kind: 'unavailable', reason: 'no-different-region' }
		: { kind: 'target', region: target };
}
