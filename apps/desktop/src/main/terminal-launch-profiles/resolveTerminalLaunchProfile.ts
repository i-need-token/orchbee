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

/** Launch context exists even when automatic shell discovery is unavailable. */
export type DesktopTerminalLaunchTemplate = Omit<TerminalLaunch, 'executable'> & {
	readonly executable: string | null;
};

export function resolveTerminalLaunchProfile(
	baseLaunch: DesktopTerminalLaunchTemplate,
	profile: DesktopTerminalLaunchProfile,
	platform: NodeJS.Platform,
	automaticShellIntegration: AutomaticShellIntegrationConfiguration | null = null,
): TerminalLaunchProfileResolution {
	const parsedProfile = DesktopTerminalLaunchProfileSchema.safeParse(profile);
	if (!parsedProfile.success) return { kind: 'rejected', reason: 'invalid-profile' };
	const command = parsedProfile.data.command;
	if (command.kind === 'system-shell') {
		if (platform === 'win32' && command.login) {
			return { kind: 'rejected', reason: 'unsupported-platform' };
		}
		const launch = TerminalLaunchSchema.safeParse({
			...baseLaunch,
			arguments: command.login ? ['-l'] : [],
		});
		return launch.success
			? resolved(launch.data, platform, automaticShellIntegration)
			: { kind: 'rejected', reason: 'invalid-profile' };
	}
	const pathImplementation = platform === 'win32' ? path.win32 : path.posix;
	if (!pathImplementation.isAbsolute(command.executable)) {
		return { kind: 'rejected', reason: 'invalid-profile' };
	}
	const launch = TerminalLaunchSchema.safeParse({
		...baseLaunch,
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
