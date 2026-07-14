import { beforeAll, describe, expect, it } from 'vitest';
import { readGhosttyBuildCapabilities } from '../build-capabilities';
import { initializeWasm } from '../../__tests__/test-helpers';
import type { WasmBridge } from '../../wasm-bridge';

describe('readGhosttyBuildCapabilities', () => {
	let bridge: WasmBridge;

	beforeAll(async () => {
		bridge = await initializeWasm();
	});

	it('reports features from the actual packaged Ghostty binary', () => {
		expect(readGhosttyBuildCapabilities(bridge)).toEqual({
			simd: false,
			kittyGraphics: false,
			tmuxControlMode: false,
		});
	});
});
