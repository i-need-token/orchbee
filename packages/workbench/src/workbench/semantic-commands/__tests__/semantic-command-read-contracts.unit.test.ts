import { describe, expect, it } from 'vitest';

import {
	WORKBENCH_DIRECTORY_LIST_OPERATION,
	WORKBENCH_PANEL_LIST_OPERATION,
	WORKBENCH_PROJECT_LIST_OPERATION,
	WORKBENCH_TAB_LIST_OPERATION,
	WORKBENCH_TERMINAL_LIST_OPERATION,
	WORKBENCH_TERMINAL_OUTPUT_DEFAULT_ROWS,
	WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES,
	WORKBENCH_TERMINAL_OUTPUT_READ_OPERATION,
	WORKBENCH_WORKSPACE_LIST_OPERATION,
	WorkbenchDirectoryListInputSchema,
	WorkbenchPanelListInputSchema,
	WorkbenchProjectListInputSchema,
	WorkbenchTabListInputSchema,
	WorkbenchTerminalListInputSchema,
	WorkbenchTerminalOutputReadInputSchema,
	WorkbenchTerminalOutputReadOutputSchema,
} from '../semantic-command-read-contracts';

describe('Workbench semantic read contracts', () => {
	it('owns seven explicit query descriptors without admitting terminal creation', () => {
		expect([
			WORKBENCH_PROJECT_LIST_OPERATION.id,
			WORKBENCH_DIRECTORY_LIST_OPERATION.id,
			WORKBENCH_WORKSPACE_LIST_OPERATION.id,
			WORKBENCH_TAB_LIST_OPERATION.id,
			WORKBENCH_PANEL_LIST_OPERATION.id,
			WORKBENCH_TERMINAL_LIST_OPERATION.id,
			WORKBENCH_TERMINAL_OUTPUT_READ_OPERATION.id,
		]).toEqual([
			'workbench.project.list',
			'workbench.directory.list',
			'workbench.workspace.list',
			'workbench.tab.list',
			'workbench.panel.list',
			'workbench.terminal.list',
			'workbench.terminal.output.read',
		]);
	});

	it('bounds exact Terminal tail reads and validates relational output metadata', () => {
		expect(
			WorkbenchTerminalOutputReadInputSchema.parse({ presentationId: 'presentation-1' }),
		).toEqual({ presentationId: 'presentation-1', rows: WORKBENCH_TERMINAL_OUTPUT_DEFAULT_ROWS });
		expect(
			WorkbenchTerminalOutputReadInputSchema.safeParse({
				presentationId: 'presentation-1',
				rows: 1_001,
			}).success,
		).toBe(false);
		const output = {
			activeScreen: 'primary',
			format: 'plain-text',
			generation: '2',
			presentationId: 'presentation-1',
			requestedRows: 80,
			returnedRows: 80,
			revision: '9',
			source: 'tail-unwrapped',
			startRow: '20',
			text: 'tail',
			totalRows: '100',
			truncated: true,
		} as const;
		expect(WorkbenchTerminalOutputReadOutputSchema.parse(output)).toEqual(output);
		expect(
			WorkbenchTerminalOutputReadOutputSchema.safeParse({ ...output, truncated: false }).success,
		).toBe(false);
		expect(
			WorkbenchTerminalOutputReadOutputSchema.safeParse({
				...output,
				text: 'x'.repeat(WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES + 1),
			}).success,
		).toBe(false);
		expect(
			WorkbenchTerminalOutputReadOutputSchema.safeParse({
				...output,
				returnedRows: 81,
			}).success,
		).toBe(false);
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
		expect(WorkbenchTabListInputSchema.parse({ location: { kind: 'standalone' } })).toEqual({
			limit: 100,
			location: { kind: 'standalone' },
		});
		expect(
			WorkbenchPanelListInputSchema.safeParse({ location: { kind: 'standalone' } }).success,
		).toBe(false);
		expect(
			WorkbenchPanelListInputSchema.safeParse({
				active: true,
				location: { kind: 'standalone' },
				tabId: 'tab-1',
			}).success,
		).toBe(false);
	});
});
