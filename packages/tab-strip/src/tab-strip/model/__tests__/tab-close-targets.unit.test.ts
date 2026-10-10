import { expect, it } from 'vitest';
import corpus from './fixtures/tab-close-targets.json';
import { getTabCloseTargets } from '../tab-close-targets';

it.each(corpus)('$name', (scenario) => {
	expect(getTabCloseTargets(scenario.tabs, new Set(scenario.selected), scenario.target)).toEqual(
		scenario.expected,
	);
});
