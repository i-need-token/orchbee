import path from 'node:path';

import { describe, expect, it } from 'vitest';

import { resolveTerminalLocalPath } from '../resolveTerminalLocalPath';

describe('resolveTerminalLocalPath', () => {
	it('expands home paths while rejecting relative and control-character targets', () => {
		const homeDirectory = path.join(path.parse(process.cwd()).root, 'orchbee-home');
		expect(resolveTerminalLocalPath('~/output.png', homeDirectory)).toBe(
			path.join(homeDirectory, 'output.png'),
		);
		expect(() => resolveTerminalLocalPath('./output.png', homeDirectory)).toThrow(
			'must be absolute',
		);
		for (const controlCharacter of ['\u0000', '\u001f', '\u007f', '\u0085', '\u009f']) {
			expect(() =>
				resolveTerminalLocalPath(`/tmp/output${controlCharacter}.png`, homeDirectory),
			).toThrow('is invalid');
		}
	});
});
