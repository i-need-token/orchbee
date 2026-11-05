import { createOrchbeeIntl } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

import { listWorkbenchAgentProviderNames } from '../../utils/workbench-agent-names';
import {
	resolveWorkbenchAgentTitle,
	type WorkbenchAgentTitleSource,
} from '../resolveWorkbenchAgentTitle';

// The TUI Agent row's rule: a person's Panel name, then the terminal's own title, then the creation
// label, skipping blank text, automatic numbers and the provider's own names.
const CLAUDE = listWorkbenchAgentProviderNames(createOrchbeeIntl('en-US'), 'claude');
const CURSOR = listWorkbenchAgentProviderNames(createOrchbeeIntl('en-US'), 'cursor');

function title(source: WorkbenchAgentTitleSource, providers = CLAUDE): string | null {
	return resolveWorkbenchAgentTitle(source, providers);
}

describe('resolveWorkbenchAgentTitle', () => {
	it('takes the first source that names the Agent', () => {
		const all = { creationLabel: 'review', customTitle: 'Login fix', naturalTask: 'Fix tests' };
		expect(title(all)).toBe('Login fix');
		expect(title({ ...all, customTitle: undefined })).toBe('Fix tests');
		expect(title({ ...all, customTitle: undefined, naturalTask: null })).toBe('review');
		expect(title({ creationLabel: '  build  ' })).toBe('build');
	});

	it('names nothing with blank text, an automatic number or only the provider', () => {
		expect(title({ creationLabel: '3', naturalTask: 'Claude Code' })).toBeNull();
		expect(
			title({ customTitle: '  ', creationLabel: 'claude-code', naturalTask: 'CLAUDE' }),
		).toBeNull();
		expect(title({ creationLabel: 'agent' }, CURSOR)).toBeNull();
		// A terminal's own title is never an automatic number, and a custom name hides the creation label.
		expect(title({ naturalTask: '42' })).toBe('42');
		expect(title({ creationLabel: 'review', customTitle: '7' })).toBeNull();
	});
});
