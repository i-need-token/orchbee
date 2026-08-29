import { describe, expect, it } from 'vitest';

import {
	SEMANTIC_COMMAND_ENDPOINT_ADDON_NAME,
	loadSemanticCommandEndpointNativeAdapter,
} from '../semantic-command-endpoint-native-adapter';

describe('semantic command endpoint native adapter loader', () => {
	it('keeps one cross-runtime addon identity and rejects relative module lookup', () => {
		expect(SEMANTIC_COMMAND_ENDPOINT_ADDON_NAME).toBe('orchbee-semantic-command-endpoint.node');
		expect(() => loadSemanticCommandEndpointNativeAdapter('relative.node')).toThrow(
			'addon path must be absolute',
		);
	});
});
