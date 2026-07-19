import { describe, expect, it } from 'vitest';

import { validateStorageSource } from '../../storage-contract.mjs';

describe('storage contract', () => {
	it('accepts browser storage injection and a canonical owner-defined key', () => {
		expect(
			validateStorageSource({
				path: 'apps/web/src/composition/entry.tsx',
				source: 'start(window.localStorage);',
			}),
		).toEqual([]);
		expect(
			validateStorageSource({
				path: 'packages/example/src/layout/example-layout-storage.ts',
				source: "const key = 'orchbee:example:panel-width:v1';",
			}),
		).toEqual([]);
	});

	it('rejects direct browser storage access outside an application entry', () => {
		const diagnostics = validateStorageSource({
			path: 'packages/example/src/panel/Panel.tsx',
			source: "window.localStorage.getItem('layout');",
		});

		expect(diagnostics[0]?.message).toContain('application composition entry');
	});

	it('rejects scattered and malformed Orchbee storage keys', () => {
		const scatteredDiagnostics = validateStorageSource({
			path: 'packages/example/src/panel/Panel.tsx',
			source: "const key = 'orchbee:example:panel-width:v1';",
		});
		const malformedDiagnostics = validateStorageSource({
			path: 'packages/example/src/layout/example-layout-storage.ts',
			source: "const key = 'orchbee:Example:panelWidth:v0';",
		});

		expect(scatteredDiagnostics[0]?.message).toContain('*-storage.ts');
		expect(malformedDiagnostics[0]?.message).toContain('orchbee:<owner>:<record>:v<schema>');
	});
});
