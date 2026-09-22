import path from 'node:path';

import { describe, expect, it } from 'vitest';

import { EnvironmentIdSchema } from '../../environment';
import { resolveDesktopRuntimeConfiguration } from '../../composition';
import { resolveDesktopSshEnvironmentTerminalRuntimeResources } from '../desktop-ssh-environment-terminal-runtime-resources';

describe('resolveDesktopSshEnvironmentTerminalRuntimeResources', () => {
	it('isolates recovery state and launches the validated remote login Shell', () => {
		const base = resolveDesktopRuntimeConfiguration({
			applicationPath: path.resolve('/repo/apps/desktop'),
			environment: {
				ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: path.resolve('/private/recovery'),
				ORCHBEE_DESKTOP_RUNTIME_DIRECTORY: path.resolve('/private/runtime'),
				SHELL: '/private/local/zsh',
			},
			homeDirectory: path.resolve('/users/local'),
			platform: process.platform,
			terminalHostName: 'orchbee-host',
			userDataPath: path.resolve('/users/local/library/orchbee'),
		});
		const environmentId = EnvironmentIdSchema.parse(`environment:${'a'.repeat(32)}`);

		const resources = resolveDesktopSshEnvironmentTerminalRuntimeResources(
			base,
			environmentId,
			'/bin/zsh',
		);

		expect(resources.configuration.terminalBindingStorePath).toBe(
			path.resolve(`/private/recovery/ssh-environments-v1/${'a'.repeat(32)}/bindings-v1.json`),
		);
		expect(resources.configuration.terminalArchiveDirectory).toBe(
			path.resolve(`/private/recovery/ssh-environments-v1/${'a'.repeat(32)}/terminal-archives-v1`),
		);
		expect(resources.configuration.terminalExitSpillPath).toBe(
			path.resolve(
				`/private/recovery/ssh-environments-v1/${'a'.repeat(32)}/terminal-exits-v1.json`,
			),
		);
		expect(resources.configuration.terminalExitPublicationLedgerPath).toBe(
			path.resolve(
				`/private/recovery/ssh-environments-v1/${'a'.repeat(32)}/terminal-exit-publications-v1.json`,
			),
		);
		expect(resources.launch).toMatchObject({
			arguments: ['-l'],
			configuration: base.terminalConfiguration,
			currentDirectory: null,
			executable: '/bin/zsh',
		});
		expect(resources.launch.environment).toEqual([
			{ name: 'COLORTERM', value: 'truecolor' },
			{ name: 'TERM', value: 'xterm-256color' },
			{ name: 'TERM_PROGRAM', value: 'orchbee' },
		]);
		expect(JSON.stringify(resources.launch)).not.toContain('/private/local');
		expect(JSON.stringify(resources.launch)).not.toContain('ORCHBEE_CLI');
	});
});
