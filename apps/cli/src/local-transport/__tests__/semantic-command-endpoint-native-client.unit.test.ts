import { describe, expect, it } from 'vitest';

import {
	SEMANTIC_COMMAND_ENDPOINT_ADDON_NAME,
	loadSemanticCommandEndpointNativeClient,
} from '../semantic-command-endpoint-native-client';

describe('semantic command endpoint native client loader', () => {
	it('keeps one cross-runtime addon identity and rejects relative module lookup', () => {
		expect(SEMANTIC_COMMAND_ENDPOINT_ADDON_NAME).toBe('orchbee-semantic-command-endpoint.node');
		expect(() => loadSemanticCommandEndpointNativeClient('relative.node')).toThrow(
			'addon path must be absolute',
		);
	});
});
