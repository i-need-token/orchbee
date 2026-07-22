import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { TabStripControlButtonEdge } from '../../../../shared';
import { TabStripControlButton } from '../TabStripControlButton';

afterEach(cleanup);

describe('TabStripControlButton', () => {
	it('renders the computed animated edge radii', () => {
		render(
			<TabStripControlButton
				aria-label='Control'
				animatedFlatEdge={TabStripControlButtonEdge.Right}
				flatEdgeFactor={0.5}
			/>,
		);
		const button = screen.getByRole('button', { name: 'Control' });
		expect(button.getAttribute('tabindex')).toBe('-1');
		expect(button.style.getPropertyValue('--ob-comp-tab-strip-corner-top-left')).toBe('14px');
		expect(button.style.getPropertyValue('--ob-comp-tab-strip-corner-top-right')).toBe('7px');
		expect(button.style.getPropertyValue('--ob-comp-tab-strip-corner-bottom-right')).toBe('7px');
	});

	it('renders the condensed top edge as square', () => {
		render(<TabStripControlButton aria-label='Control' frameCondensed />);
		const button = screen.getByRole('button', { name: 'Control' });
		expect(button.style.getPropertyValue('--ob-comp-tab-strip-corner-top-left')).toBe('0px');
		expect(button.style.getPropertyValue('--ob-comp-tab-strip-corner-top-right')).toBe('0px');
		expect(button.getAttribute('data-frame-condensed')).toBe('true');
	});
});
