import { isValidElement } from 'react';
import { describe, expect, it } from 'vitest';

import { TooltipProvider } from '../tooltip';

describe('TooltipProvider', () => {
	it('defaults the shared open delay to 300 milliseconds', () => {
		const provider = TooltipProvider({ children: null });
		if (!isValidElement<{ readonly delay?: number }>(provider)) {
			throw new Error('TooltipProvider must return a React element.');
		}

		expect(provider.props.delay).toBe(300);
	});
});
