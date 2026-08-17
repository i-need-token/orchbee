// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { useMacosTerminalInputSettings } from '../useMacosTerminalInputSettings';
import { MacosTerminalInputSettingsController } from '../../model';

afterEach(cleanup);

describe('useMacosTerminalInputSettings', () => {
	it('projects complete controller snapshots through the external-store contract', () => {
		const controller = new MacosTerminalInputSettingsController({
			getItem: () => null,
			setItem: () => undefined,
		});
		const { result, unmount } = renderHook(() => useMacosTerminalInputSettings(controller));

		expect(result.current.preferences).toEqual({ optionAsAlt: 'off' });
		act(() => {
			controller.update({ optionAsAlt: 'both' });
		});
		expect(result.current.preferences).toEqual({ optionAsAlt: 'both' });

		unmount();
		controller.dispose();
	});
});
