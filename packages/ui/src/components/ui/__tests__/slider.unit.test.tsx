import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { Slider } from '../slider';

describe('Slider', () => {
	it('forwards the group label to its interactive thumb', () => {
		const markup = renderToStaticMarkup(
			<Slider aria-label='Terminal opacity' max={100} min={10} value={[65]} />,
		);

		expect(markup).toContain('role="group"');
		expect(markup).toMatch(/<input[^>]*aria-label="Terminal opacity"[^>]*type="range"/u);
	});
});
