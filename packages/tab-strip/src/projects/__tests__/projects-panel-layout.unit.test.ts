import { describe, expect, it } from 'vitest';
import {
	calculateProjectsPanelLayout,
	getProjectsPanelCloseButtonOpacity,
} from '../projects-panel-layout';

describe('projects panel layout calculations', () => {
	it('exports the calculations exercised by projects-panel behavior tests', () => {
		expect(calculateProjectsPanelLayout).toBeTypeOf('function');
		expect(getProjectsPanelCloseButtonOpacity).toBeTypeOf('function');
	});
});
