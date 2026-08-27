import path from 'node:path';

import {
	DesktopTerminalLaunchProfileSchema,
	type DesktopTerminalLaunchProfile,
} from '@orchbee/desktop-client-protocol';
import { TerminalLaunchSchema, type TerminalLaunch } from '@orchbee/terminal-protocol';

import {
	resolveAutomaticShellIntegration,
	type AutomaticShellIntegrationConfiguration,
} from './resolveAutomaticShellIntegration';

export type TerminalLaunchProfileResolution =
	| { readonly kind: 'resolved'; readonly launch: TerminalLaunch }
	| { readonly kind: 'rejected'; readonly reason: 'invalid-profile' | 'unsupported-platform' };

export function resolveTerminalLaunchProfile(
	baseLaunch: TerminalLaunch,
	profile: DesktopTerminalLaunchProfile,
	platform: NodeJS.Platform,
	automaticShellIntegration: AutomaticShellIntegrationConfiguration | null = null,
): TerminalLaunchProfileResolution {
	const trustedBase = TerminalLaunchSchema.parse(baseLaunch);
	const parsedProfile = DesktopTerminalLaunchProfileSchema.safeParse(profile);
	if (!parsedProfile.success) return { kind: 'rejected', reason: 'invalid-profile' };
	const command = parsedProfile.data.command;
	if (command.kind === 'system-shell') {
		if (platform === 'win32' && command.login) {
			return { kind: 'rejected', reason: 'unsupported-platform' };
		}
		return resolved(
			TerminalLaunchSchema.parse({
				...trustedBase,
				arguments: command.login ? ['-l'] : [],
			}),
			platform,
			automaticShellIntegration,
		);
	}
	const pathImplementation = platform === 'win32' ? path.win32 : path.posix;
	if (!pathImplementation.isAbsolute(command.executable)) {
		return { kind: 'rejected', reason: 'invalid-profile' };
	}
	const launch = TerminalLaunchSchema.safeParse({
		...trustedBase,
		arguments: command.arguments,
		executable: command.executable,
	});
	return launch.success
		? resolved(launch.data, platform, automaticShellIntegration)
		: { kind: 'rejected', reason: 'invalid-profile' };
}

function resolved(
	launch: TerminalLaunch,
	platform: NodeJS.Platform,
	automaticShellIntegration: AutomaticShellIntegrationConfiguration | null,
): TerminalLaunchProfileResolution {
	return {
		kind: 'resolved',
		launch: resolveAutomaticShellIntegration(launch, platform, automaticShellIntegration).launch,
	};
}
