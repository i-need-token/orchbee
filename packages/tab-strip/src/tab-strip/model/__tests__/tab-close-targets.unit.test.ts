import { expect, it } from 'vitest';
import corpus from '../../../../../../schemas/workbench/v1/tab-close-targets-conformance-v1.json';
import { getTabCloseTargets } from '../tab-close-targets';

it('reads the version-1 shared Tab Close corpus', () => {
	expect(corpus.contractVersion).toBe(1);
});

it.each(corpus.cases)('$name', (scenario) => {
	expect(getTabCloseTargets(scenario.tabs, new Set(scenario.selected), scenario.target)).toEqual(
		scenario.expected,
	);
});
