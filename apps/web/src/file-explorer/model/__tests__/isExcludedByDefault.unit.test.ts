import { describe, expect, it } from 'vitest';

import { isExcludedByDefault } from '../isExcludedByDefault';
import fixture from './fixtures/vscode-default-excludes.json';

// The pinned VS Code's verdicts for each name, under the default files.exclude of its desktop build.
describe('isExcludedByDefault', () => {
	it.each(fixture.excluded)('hides %j as VS Code does', (name) => {
		expect(isExcludedByDefault(name)).toBe(true);
	});

	it.each(fixture.kept)('shows %j as VS Code does', (name) => {
		expect(isExcludedByDefault(name)).toBe(false);
	});
});
