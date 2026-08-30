import type { DesktopDistributionProfile } from '../src/distribution';

export function materializeDarwinApplicationIcon(options: {
	readonly baseIconSvg: string;
	readonly outputDirectory: string;
	readonly profile: DesktopDistributionProfile;
}): Promise<string>;
