import { describe, expect, it } from 'vitest';

import { observeTerminalHostBuild } from '../observeTerminalHostBuild';

describe('observeTerminalHostBuild', () => {
	it('publishes a newer Host binary even when its observed input digest is unchanged', async () => {
		let binaryDigest = 'binary-generation-one';
		const builds: Array<{ readonly digest: string; readonly inputDigest: string }> = [];
		const observe = (currentDigest: string) =>
			observeTerminalHostBuild({
				binaryPath: '/repo/apps/terminal-host/zig-out/bin/orchbee-terminal-host',
				currentDigest,
				onBuild: async (build) => {
					builds.push(build);
				},
				readBinaryDigest: async () => binaryDigest,
				readInputDigest: async () => 'source-generation-two',
			});

		let currentDigest = await observe('binary-generation-zero');
		binaryDigest = 'binary-generation-two';
		currentDigest = await observe(currentDigest);

		expect(builds).toHaveLength(2);
		expect(builds[0]?.inputDigest).toBe(builds[1]?.inputDigest);
		expect(builds[0]?.digest).not.toBe(builds[1]?.digest);
		expect(currentDigest).toBe(builds[1]?.digest);
	});
});
