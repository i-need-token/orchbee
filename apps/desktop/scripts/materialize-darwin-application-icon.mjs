import { execFile } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { promisify } from 'node:util';

import { renderDesktopApplicationIconSvg } from '../src/distribution/tooling/index.ts';

const executeFile = promisify(execFile);
const DARWIN_ICON_RENDITIONS = [
	['icon_16x16.png', 16],
	['icon_16x16@2x.png', 32],
	['icon_32x32.png', 32],
	['icon_32x32@2x.png', 64],
	['icon_128x128.png', 128],
	['icon_128x128@2x.png', 256],
	['icon_256x256.png', 256],
	['icon_256x256@2x.png', 512],
	['icon_512x512.png', 512],
	['icon_512x512@2x.png', 1024],
];

export async function materializeDarwinApplicationIcon(options) {
	if (process.platform !== 'darwin') {
		throw new Error('Desktop .icns materialization is available only on macOS.');
	}
	const iconRoot = path.resolve(options.outputDirectory);
	const iconSetPath = path.join(iconRoot, `${options.profile.profileId}.iconset`);
	const renderedSvgPath = path.join(iconRoot, `${options.profile.profileId}.svg`);
	const outputPath = path.join(iconRoot, `${options.profile.profileId}.icns`);
	await mkdir(iconSetPath, { mode: 0o700, recursive: true });
	await writeFile(
		renderedSvgPath,
		renderDesktopApplicationIconSvg(options.baseIconSvg, options.profile),
		{ encoding: 'utf8', mode: 0o600 },
	);
	await Promise.all(
		DARWIN_ICON_RENDITIONS.map(async ([fileName, size]) => {
			await executeFile(
				'/usr/bin/sips',
				[
					'-s',
					'format',
					'png',
					'-z',
					String(size),
					String(size),
					renderedSvgPath,
					'--out',
					path.join(iconSetPath, fileName),
				],
				{ encoding: 'utf8' },
			);
		}),
	);
	await executeFile('/usr/bin/iconutil', ['-c', 'icns', iconSetPath, '-o', outputPath], {
		encoding: 'utf8',
	});
	return outputPath;
}
