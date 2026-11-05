import { createOrchbeeIntl } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

import type { WorkbenchAgentNames } from '../../types/workbench-agent-observation';
import { listWorkbenchAgentProviderNames } from '../../utils/workbench-agent-names';
import {
	resolveWorkbenchAgentTitle,
	type WorkbenchAgentTitleSource,
} from '../resolveWorkbenchAgentTitle';

// The TUI Agent row's rule: the name a person gave the Agent, then a reported title or else a
// person's Panel name, then the terminal's own title, then the creation label, skipping blank text,
// automatic numbers and the provider's own names.
const CLAUDE = listWorkbenchAgentProviderNames(createOrchbeeIntl('en-US'), 'claude');
const CURSOR = listWorkbenchAgentProviderNames(createOrchbeeIntl('en-US'), 'cursor');

type Reported = Pick<WorkbenchAgentNames, 'name' | 'reportedTitle' | 'terminalTitle'>;

const TERMINAL = { creationLabel: 'review', customTitle: 'Login fix', naturalTask: 'Fix tests' };
const UNNAMED: Reported = { name: null, reportedTitle: null, terminalTitle: null };

function title(
	terminal: WorkbenchAgentTitleSource,
	reported?: Reported,
	providers = CLAUDE,
): string | null {
	return resolveWorkbenchAgentTitle(terminal, reported, providers);
}

describe('resolveWorkbenchAgentTitle', () => {
	it('takes the first source that names the Agent, in the TUI order', () => {
		const reported = { name: 'scout', reportedTitle: 'Ship login', terminalTitle: 'Run tests' };
		expect(title(TERMINAL, reported)).toBe('scout');
		expect(title(TERMINAL, { ...reported, name: null })).toBe('Ship login');
		// Without a reported title, a person's Panel name stands in its place.
		expect(title(TERMINAL, { ...reported, name: null, reportedTitle: null })).toBe('Login fix');
		const uncustomized = { ...TERMINAL, customTitle: undefined };
		expect(title(uncustomized, { ...UNNAMED, terminalTitle: 'Run tests' })).toBe('Run tests');
		expect(title(uncustomized, UNNAMED)).toBe('review');
		// Until the Server has read the terminal's title, the one this window saw stands in.
		expect(title(uncustomized, { ...UNNAMED, terminalTitle: undefined })).toBe('Fix tests');
		expect(title(uncustomized)).toBe('Fix tests');
		expect(title({ creationLabel: '  build  ' })).toBe('build');
	});

	it('names nothing with blank text, an automatic number or only the provider', () => {
		expect(title({ creationLabel: '3', naturalTask: 'Claude Code' })).toBeNull();
		expect(
			title({ customTitle: '  ', creationLabel: 'claude-code', naturalTask: 'CLAUDE' }),
		).toBeNull();
		expect(title({ creationLabel: 'agent' }, undefined, CURSOR)).toBeNull();
		// A terminal's own title is never an automatic number, and a custom name hides the creation label.
		expect(title({ naturalTask: '42' })).toBe('42');
		expect(title({ creationLabel: 'review', customTitle: '7' })).toBeNull();
		// A reported title that only names the provider hides the Panel name too, as in the TUI.
		expect(title(TERMINAL, { ...UNNAMED, reportedTitle: 'Claude Code', terminalTitle: '12' })).toBe(
			'12',
		);
		expect(title({ customTitle: 'Login fix' }, { ...UNNAMED, reportedTitle: '5' })).toBeNull();
	});
});
