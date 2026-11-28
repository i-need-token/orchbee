import type { ScmRepositoryState } from './ScmWorkspace';
import {
	SCM_STATUS_FACTS,
	type ScmResource,
	type ScmResourceStatus,
	type ScmStatusColor,
} from './scm-resource';

/** A path's Git decoration, as VS Code's Git extension gives it to the Explorer. */
export interface ScmFileDecoration {
	readonly color: ScmStatusColor;
	readonly letter: string;
	/** The folders above the path take its color as well, as for every status but a deletion. */
	readonly propagates: boolean;
	/** The status the decoration's hover text names. */
	readonly status: ScmResourceStatus;
}

/** VS Code's collecting order: a later group's decoration replaces an earlier one's on a path. */
const DECORATED_GROUPS = ['index', 'workingTree', 'merge'] as const;

function collect(resource: ScmResource, decorations: Map<string, ScmFileDecoration>): void {
	const { status } = resource;
	const { color, letter } = SCM_STATUS_FACTS[status];
	const decoration: ScmFileDecoration = {
		color,
		letter,
		propagates: status !== 'deleted' && status !== 'index-deleted',
		status,
	};
	decorations.set(resource.original ?? resource.path, decoration);
	if (status === 'index-renamed' || status === 'intent-to-rename') {
		decorations.set(resource.path, decoration);
	}
}

/**
 * Each changed path's decoration across the repositories, as VS Code's Git extension collects them
 * after a status: Staged Changes, then Changes, then Merge Changes, a later group's decoration
 * replacing an earlier one's on the same path. A row decorates the path its change came from, as
 * VS Code's resource keeps it, which for a rename or a copy is its source; a rename decorates its
 * new path as well.
 */
export function scmFileDecorations(
	repositories: readonly ScmRepositoryState[],
): ReadonlyMap<string, ScmFileDecoration> {
	const decorations = new Map<string, ScmFileDecoration>();
	for (const { groups } of repositories) {
		if (groups === null) continue;
		for (const group of DECORATED_GROUPS) {
			for (const resource of groups[group]) collect(resource, decorations);
		}
	}
	return decorations;
}
