import { readFileSync, realpathSync } from 'node:fs';
import path from 'node:path';

import {
	DESKTOP_DISTRIBUTION_DESCRIPTOR_FILE_NAME,
	createDesktopDevelopmentScope,
	parseDesktopDistributionDescriptor,
	resolveDesktopDistributionDescriptor,
	resolveDesktopDistributionProfile,
	type DesktopDistributionProfile,
} from '../../distribution';

type DesktopApplicationDistributionEnvironment = {
	readonly ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT?: string;
};

interface DesktopApplicationDistributionInput {
	readonly applicationPath: string;
	readonly environment: DesktopApplicationDistributionEnvironment;
	readonly packaged: boolean;
	readonly platform: NodeJS.Platform;
}

interface DesktopApplicationDistributionDependencies {
	readonly readFile: (filePath: string, encoding: 'utf8') => string;
	readonly realpath: (filePath: string) => string;
}

const defaultDependencies: DesktopApplicationDistributionDependencies = {
	readFile: (filePath, encoding) => readFileSync(filePath, encoding),
	realpath: realpathSync.native,
};

export function resolveDesktopApplicationDistribution(
	input: DesktopApplicationDistributionInput,
	dependencies: DesktopApplicationDistributionDependencies = defaultDependencies,
): DesktopDistributionProfile {
	if (!input.packaged) {
		const configuredSourceRoot = input.environment.ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT;
		if (configuredSourceRoot === undefined || configuredSourceRoot.length === 0) {
			throw new Error('Development Desktop requires its canonical source checkout root.');
		}
		const sourceRoot = dependencies.realpath(configuredSourceRoot);
		return resolveDesktopDistributionProfile(
			'dev',
			createDesktopDevelopmentScope(sourceRoot, input.platform),
		);
	}

	const platformPath = input.platform === 'win32' ? path.win32 : path.posix;
	if (!platformPath.isAbsolute(input.applicationPath)) {
		throw new Error('Packaged Desktop application path must be absolute.');
	}
	const descriptor = parseDesktopDistributionDescriptor(
		JSON.parse(
			dependencies.readFile(
				platformPath.join(input.applicationPath, DESKTOP_DISTRIBUTION_DESCRIPTOR_FILE_NAME),
				'utf8',
			),
		),
	);
	if (descriptor.profileId !== 'dev') {
		return resolveDesktopDistributionDescriptor(descriptor, input.platform);
	}
	const sourceRoot = dependencies.realpath(descriptor.developmentScope.sourceRoot);
	const currentScope = createDesktopDevelopmentScope(sourceRoot, input.platform);
	if (
		currentScope.sourceRoot !== descriptor.developmentScope.sourceRoot ||
		currentScope.digest !== descriptor.developmentScope.digest
	) {
		throw new Error(
			'The packaged Desktop dev distribution does not match its current source checkout.',
		);
	}
	return resolveDesktopDistributionProfile('dev', currentScope);
}
