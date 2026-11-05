// Regenerates the GUI's color-theme files from crates/color-themes. The crate's staleness test is
// the generator: with ORCHBEE_GENERATE_COLOR_THEMES set it writes the files instead of comparing.
// `--out-dir <dir>` writes there instead of the tracked paths, and each `--theme <file>` adds an
// unshipped theme file to that output for review; `--theme` requires `--out-dir`.
import { spawnSync } from 'node:child_process';
import { delimiter, resolve } from 'node:path';

const repositoryRoot = resolve(import.meta.dirname, '..');
const themes = [];
let outDirectory;
const argumentsList = process.argv.slice(2);
for (let index = 0; index < argumentsList.length; index += 1) {
	const argument = argumentsList[index];
	const value = argumentsList[index + 1];
	if ((argument === '--theme' || argument === '--out-dir') && value !== undefined) {
		if (argument === '--theme') themes.push(resolve(value));
		else outDirectory = resolve(value);
		index += 1;
	} else {
		throw new Error(`Unsupported argument: ${argument}`);
	}
}
if (themes.length > 0 && outDirectory === undefined) {
	throw new Error(
		'--theme renders only into --out-dir; shipped themes live in crates/color-themes.',
	);
}

const result = spawnSync(
	'cargo',
	[
		'test',
		'--quiet',
		'--package',
		'orchbee-color-themes',
		'--test',
		'integration',
		'--',
		'--exact',
		'client_files::generated_client_files_match_the_shipped_themes',
	],
	{
		cwd: repositoryRoot,
		env: {
			...process.env,
			ORCHBEE_GENERATE_COLOR_THEMES: '1',
			...(outDirectory === undefined ? {} : { ORCHBEE_COLOR_THEMES_OUT_DIR: outDirectory }),
			...(themes.length === 0 ? {} : { ORCHBEE_COLOR_THEMES_EXTRA: themes.join(delimiter) }),
		},
		stdio: 'inherit',
	},
);
if (result.error !== undefined) throw result.error;
process.exitCode = result.status ?? 1;
