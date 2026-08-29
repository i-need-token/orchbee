import { describe, expect, it } from 'vitest';

import {
	WORKBENCH_DIRECTORY_LIST_OPERATION,
	WORKBENCH_PROJECT_LIST_OPERATION,
	WORKBENCH_TERMINAL_LIST_OPERATION,
	WORKBENCH_WORKSPACE_LIST_OPERATION,
	WorkbenchDirectoryListInputSchema,
	WorkbenchProjectListInputSchema,
	WorkbenchTerminalListInputSchema,
} from '../semantic-command-read-contracts';

describe('Workbench semantic read contracts', () => {
	it('owns four explicit query descriptors without admitting terminal creation', () => {
		expect([
			WORKBENCH_PROJECT_LIST_OPERATION.id,
			WORKBENCH_DIRECTORY_LIST_OPERATION.id,
			WORKBENCH_WORKSPACE_LIST_OPERATION.id,
			WORKBENCH_TERMINAL_LIST_OPERATION.id,
		]).toEqual([
			'workbench.project.list',
			'workbench.directory.list',
			'workbench.workspace.list',
			'workbench.terminal.list',
		]);
	});

	it('defaults and bounds PageInput while rejecting unknown and implicit target fields', () => {
		expect(WorkbenchProjectListInputSchema.parse({})).toEqual({ limit: 100 });
		expect(WorkbenchProjectListInputSchema.parse({ limit: 200 })).toEqual({ limit: 200 });
		expect(WorkbenchProjectListInputSchema.safeParse({ limit: 201 }).success).toBe(false);
		expect(
			WorkbenchDirectoryListInputSchema.safeParse({ projectId: 'project-1', active: true }).success,
		).toBe(false);
		expect(
			WorkbenchTerminalListInputSchema.safeParse({ location: { kind: 'workspace' } }).success,
		).toBe(false);
	});
});
