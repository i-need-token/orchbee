import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { TabStripControlButtonEdge } from '../../../../../shared';
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
		expect(button.style.borderTopLeftRadius).toBe('14px');
		expect(button.style.borderTopRightRadius).toBe('7px');
		expect(button.style.borderBottomRightRadius).toBe('7px');
	});

	it('renders the condensed top edge as square', () => {
		render(<TabStripControlButton aria-label='Control' frameCondensed />);
		const button = screen.getByRole('button', { name: 'Control' });
		expect(button.style.borderTopLeftRadius).toBe('0px');
		expect(button.style.borderTopRightRadius).toBe('0px');
		expect(button.getAttribute('data-frame-condensed')).toBe('true');
	});
});
