import { describe, expect, it } from 'vitest';

import { resolveTerminalMetadataObservation } from '../resolveTerminalMetadataObservation';

describe('terminal metadata policy', () => {
	it('validates OSC 7 hostname and percent-decoding rules', () => {
		expect(
			resolveTerminalMetadataObservation(
				{
					currentDirectory: 'file://Orchbee-Host/workspace/My%20Project',
					title: 'vim\0',
				},
				'orchbee-host',
			),
		).toEqual({
			currentDirectory: { currentCwd: '/workspace/My Project', kind: 'reported' },
			naturalTitle: 'vim',
		});
		expect(
			resolveTerminalMetadataObservation({ currentDirectory: null, title: null }, 'orchbee-host'),
		).toEqual({
			currentDirectory: { currentCwd: null, kind: 'reported' },
			naturalTitle: null,
		});
		expect(
			resolveTerminalMetadataObservation(
				{ currentDirectory: null, title: ' \0 natural  ' },
				'orchbee-host',
			),
		).toMatchObject({ naturalTitle: ' natural' });
	});

	it.each([
		['file://Orchbee-Host/C:/Users/My%20Name', 'C:\\Users\\My Name'],
		['file:///C:/Windows', 'C:\\Windows'],
		['file://localhost/D:/', 'D:\\'],
		['file://orchbee-host/C:\\Windows', 'C:\\Windows'],
	])('reports %s in the form Windows accepts', (currentDirectory, currentCwd) => {
		expect(
			resolveTerminalMetadataObservation({ currentDirectory, title: null }, 'orchbee-host')
				.currentDirectory,
		).toEqual({ currentCwd, kind: 'reported' });
	});

	it.each([
		'file://remote-host/C:/Windows',
		'file:///workspace',
		'file://localhost/workspace',
		'file://remote-host/workspace',
		'file://orchbee-host/workspace/%',
		'file://orchbee-host/workspace/%FF',
		'https://orchbee-host/workspace',
	])('ignores an untrusted or malformed OSC 7 value: %s', (currentDirectory) => {
		expect(
			resolveTerminalMetadataObservation(
				{ currentDirectory, title: '  natural title\0  ' },
				'orchbee-host',
			),
		).toEqual({
			currentDirectory: { kind: 'ignored' },
			naturalTitle: 'natural title',
		});
	});
});
