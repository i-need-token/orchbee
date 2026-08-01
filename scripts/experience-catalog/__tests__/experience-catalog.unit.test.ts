import { describe, expect, it } from 'vitest';
import {
	experienceComponentSchema,
	filterCatalog,
	loadCatalog,
	renderSummary,
} from '../../experience-catalog.mjs';

const manifest = experienceComponentSchema.parse({
	$schema: '../../../../schemas/experience-component.schema.json',
	avoidWhen: ['The interaction is a single primitive with no product policy.'],
	description: 'A reviewed resource editing pattern with explicit lifecycle and action ownership.',
	id: 'workbench.resource-editor',
	implementation: ['packages/workbench/src/resource-editor/ResourceEditor.tsx'],
	intents: ['edit-resource', 'manage-resource'],
	invariants: ['Cancellation never mutates the resource owned by the feature.'],
	kind: 'experience',
	name: 'Resource Editor',
	owner: '@orchbee/workbench',
	references: ['packages/workbench/docs/resource-editor.md'],
	status: 'canonical',
	stories: [
		{
			exportName: 'Default',
			file: 'packages/workbench/stories/resource-editor.stories.tsx',
			id: 'packages-workbench-resource-editor--default',
			states: ['Open the editor and inspect the initial resource form.'],
		},
	],
	useWhen: ['A Workbench-owned resource needs reviewed editing behavior.'],
	uses: [],
	visibility: 'private',
});
const record = {
	manifest,
	path: '/repository/packages/workbench/docs/experience/resource-editor.json',
};

describe('experience catalog', () => {
	it('requires canonical components to reference a Story', () => {
		const result = experienceComponentSchema.safeParse({ ...manifest, stories: [] });

		expect(result.success).toBe(false);
	});

	it('finds components through deterministic terms and exact intent filters', () => {
		expect(
			filterCatalog([record], {
				filters: { intent: 'manage-resource' },
				terms: ['editor'],
			}),
		).toEqual([record]);
		expect(
			filterCatalog([record], {
				filters: { intent: 'create-resource' },
				terms: [],
			}),
		).toEqual([]);
	});

	it('renders a compact progressive-disclosure summary', () => {
		expect(renderSummary([record])).toContain('workbench.resource-editor');
		expect(renderSummary([record])).toContain(manifest.description);
		expect(renderSummary([])).toBe('No matching experience components.\n');
	});

	it('loads the checked-in catalog with resolvable source and Story evidence', async () => {
		const catalog = await loadCatalog();

		expect(catalog.map(({ manifest: entry }) => entry.id)).toEqual(
			expect.arrayContaining([
				'workbench.action-menu',
				'workbench.project-workspace-navigation',
				'workbench.resource-dialog-form',
			]),
		);
	});
});
