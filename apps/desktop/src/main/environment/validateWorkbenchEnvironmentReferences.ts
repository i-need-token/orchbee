import type { WorkbenchSharedDocument } from '@orchbee/workbench/document';

import { EnvironmentIdSchema, type EnvironmentId } from './environment-registry-contract';

export function validateWorkbenchEnvironmentReferences(
	document: WorkbenchSharedDocument,
	registeredEnvironmentIds: ReadonlySet<EnvironmentId>,
): boolean {
	for (const project of document.projectWorkspace.projects) {
		for (const directory of project.directories) {
			if (directory.placement.kind !== 'environment') continue;
			const parsed = EnvironmentIdSchema.safeParse(directory.placement.environmentId);
			if (!parsed.success || !registeredEnvironmentIds.has(parsed.data)) return false;
		}
	}
	return true;
}
