import { Children, isValidElement, type ReactNode } from 'react';
import { describe, expect, it } from 'vitest';

import { TooltipContent, TooltipProvider } from '../tooltip';

describe('TooltipProvider', () => {
	it('defaults the shared open delay to 300 milliseconds', () => {
		const provider = TooltipProvider({ children: null });
		if (!isValidElement<{ readonly delay?: number }>(provider)) {
			throw new Error('TooltipProvider must return a React element.');
		}

		expect(provider.props.delay).toBe(300);
	});

	it('omits a directional arrow from shared tooltip content', () => {
		const portal = TooltipContent({ children: 'Create Project' });
		if (!isValidElement<{ readonly children?: ReactNode }>(portal)) {
			throw new Error('TooltipContent must return a React element.');
		}

		const positioner = portal.props.children;
		if (!isValidElement<{ readonly children?: ReactNode }>(positioner)) {
			throw new Error('TooltipContent must render a positioner.');
		}

		const popup = positioner.props.children;
		if (!isValidElement<{ readonly children?: ReactNode }>(popup)) {
			throw new Error('TooltipContent must render a popup.');
		}

		expect(Children.toArray(popup.props.children)).toEqual(['Create Project']);
	});
});
