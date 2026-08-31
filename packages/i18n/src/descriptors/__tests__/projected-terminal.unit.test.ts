import { describe, expect, it } from 'vitest';

import { enUSProjectedTerminalMessages } from '../../locales';
import { projectedTerminalMessages } from '../projected-terminal';

describe('projectedTerminalMessages', () => {
	it('uses canonical English defaults for every projected-terminal descriptor', () => {
		const descriptorMessages = Object.fromEntries(
			Object.values(projectedTerminalMessages).map((descriptor) => [
				descriptor.id,
				descriptor.defaultMessage,
			]),
		);
		expect(descriptorMessages).toEqual(enUSProjectedTerminalMessages);
	});
});
