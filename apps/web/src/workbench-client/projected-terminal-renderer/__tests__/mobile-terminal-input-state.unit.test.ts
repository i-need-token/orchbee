import { describe, expect, it } from 'vitest';

import {
	clearMobileTerminalModifiers,
	consumeMobileTerminalInput,
	createMobileTerminalInputState,
	replaceMobileTerminalInputReadiness,
	setMobileTerminalInputFocus,
	toggleMobileTerminalModifier,
} from '../mobile-terminal-input-state';

describe('mobile terminal input state', () => {
	it('arms each modifier only while an exact attachment revision is ready', () => {
		const unavailable = createMobileTerminalInputState();
		expect(toggleMobileTerminalModifier(unavailable, 'control')).toBe(unavailable);

		const ready = replaceMobileTerminalInputReadiness(unavailable, {
			kind: 'ready',
			revision: 'attachment:3',
		});
		const control = toggleMobileTerminalModifier(ready, 'control');
		const both = toggleMobileTerminalModifier(control, 'alt');
		expect(both).toMatchObject({ alt: true, control: true });
		expect(toggleMobileTerminalModifier(both, 'control')).toMatchObject({
			alt: true,
			control: false,
		});
	});

	it('consumes an immutable revision snapshot before publication', () => {
		let state = createMobileTerminalInputState({ kind: 'ready', revision: 'attachment:7' }, true);
		state = toggleMobileTerminalModifier(state, 'control');
		state = toggleMobileTerminalModifier(state, 'alt');
		const consumption = consumeMobileTerminalInput(state);

		expect(consumption.snapshot).toEqual({
			alt: true,
			control: true,
			revision: 'attachment:7',
		});
		expect(consumption.state).toMatchObject({ alt: false, control: false, focused: true });
	});

	it('clears one-shot state on blur, revision replacement, downgrade, and explicit repair', () => {
		const armed = toggleMobileTerminalModifier(
			toggleMobileTerminalModifier(
				createMobileTerminalInputState({ kind: 'ready', revision: 'attachment:1' }, true),
				'control',
			),
			'alt',
		);
		expect(setMobileTerminalInputFocus(armed, false)).toMatchObject({
			alt: false,
			control: false,
			focused: false,
		});
		expect(
			replaceMobileTerminalInputReadiness(armed, {
				kind: 'ready',
				revision: 'attachment:2',
			}),
		).toMatchObject({ alt: false, control: false });
		expect(replaceMobileTerminalInputReadiness(armed, { kind: 'unavailable' })).toMatchObject({
			alt: false,
			control: false,
		});
		expect(clearMobileTerminalModifiers(armed)).toMatchObject({ alt: false, control: false });
	});
});
