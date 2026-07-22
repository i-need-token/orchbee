import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AlertIndicatorButton } from '../AlertIndicatorButton';

afterEach(cleanup);

describe('AlertIndicatorButton', () => {
	it('toggles audio immediately when the active tab owns the click', () => {
		const onToggle = vi.fn<() => void>();
		render(
			<AlertIndicatorButton
				alertState='audio-playing'
				active
				largestSelectableRegionWidth={0}
				onToggleAudioMute={onToggle}
			/>,
		);
		fireEvent.click(screen.getByRole('button', { name: 'Mute tab audio' }));
		expect(onToggle).toHaveBeenCalledWith(true);
		expect(screen.getByRole('button', { name: 'Unmute tab audio' })).toBeTruthy();
	});

	it('disables click-to-mute on a narrow inactive tab', () => {
		render(
			<AlertIndicatorButton
				alertState='audio-playing'
				active={false}
				largestSelectableRegionWidth={39}
			/>,
		);
		expect(screen.getByRole<HTMLButtonElement>('button', { name: 'Mute tab audio' }).disabled).toBe(
			true,
		);
	});

	it('renders camera, capture, device, and actor alert variants', () => {
		const { rerender, container } = render(
			<AlertIndicatorButton alertState='video-recording' active largestSelectableRegionWidth={0} />,
		);
		expect(
			screen
				.getByRole('button', { name: 'This tab is using your camera' })
				.getAttribute('data-alert-color'),
		).toBe('recording');

		rerender(
			<AlertIndicatorButton alertState='tab-capturing' active largestSelectableRegionWidth={0} />,
		);
		expect(
			screen
				.getByRole('button', { name: 'This tab is being shared' })
				.getAttribute('data-alert-color'),
		).toBe('pip');

		rerender(
			<AlertIndicatorButton alertState='usb-connected' active largestSelectableRegionWidth={0} />,
		);
		expect(
			screen
				.getByRole('button', { name: 'This tab is connected to a USB device' })
				.getAttribute('data-alert-color'),
		).toBe('default');

		rerender(
			<AlertIndicatorButton alertState='actor-accessing' active largestSelectableRegionWidth={0} />,
		);
		expect(screen.getByRole('button', { name: 'This tab is being controlled' })).toBeTruthy();
		expect(container.querySelector('.ct-alert-actor-spinner')).toBeTruthy();
	});
});
