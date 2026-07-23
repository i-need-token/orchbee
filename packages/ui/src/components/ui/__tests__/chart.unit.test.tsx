import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import { ChartStyle } from '../chart';

describe('ChartStyle', () => {
	it('targets the explicit dark data-theme boundary', () => {
		const markup = renderToStaticMarkup(
			<ChartStyle
				id='theme-contract'
				config={{
					series: {
						theme: {
							light: 'white',
							dark: 'black',
						},
					},
				}}
			/>,
		);

		expect(markup).toContain("[data-theme='dark'] [data-chart=theme-contract]");
		expect(markup).not.toContain('.dark');
	});
});
