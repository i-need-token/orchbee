import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

import { modelWorkers } from './model-checks.mjs';
import {
	applyConfigConstants,
	parseStateReport,
	variantConfigFileName,
} from './model-inventory.mjs';

export function sha256Bytes(bytes) {
	return createHash('sha256').update(bytes).digest('hex');
}

export async function executionPlans(models, repository, heapMiB, timeoutFor) {
	const plans = [];
	for (const model of models) {
		const sources = await Promise.all(
			[model.module, ...(model.supportingModules ?? [])].map(async (sourcePath) => ({
				path: sourcePath,
				bytes: await readFile(path.join(repository, sourcePath)),
			})),
		);
		const baseConfig = await readFile(path.join(repository, model.config));
		for (const variant of model.variants ?? [{ name: 'default' }]) {
			const config =
				variant.constants === undefined
					? baseConfig
					: Buffer.from(applyConfigConstants(baseConfig.toString('utf8'), variant.constants));
			const digest = createHash('sha256');
			for (const source of sources)
				digest.update(source.path).update('\0').update(source.bytes).update('\0');
			digest.update(model.config).update('\0').update(config);
			plans.push({
				model,
				variant,
				sources,
				config,
				configName:
					variant.constants === undefined
						? path.basename(model.config)
						: variantConfigFileName(model.config, variant.name),
				proof: {
					model: model.id,
					variant: variant.name,
					modelConfigSha256: digest.digest('hex'),
					workers: modelWorkers(model),
					heapMiB,
					timeoutMilliseconds: timeoutFor(model),
				},
			});
		}
	}
	return plans;
}

export async function stageInputs(plan, directory) {
	const files = [
		...plan.sources.map((source) => ({ name: path.basename(source.path), bytes: source.bytes })),
		{ name: plan.configName, bytes: plan.config },
	];
	if (new Set(files.map((file) => file.name)).size !== files.length)
		throw new Error('TLC input filenames must be distinct');
	for (const file of files)
		await writeFile(path.join(directory, file.name), file.bytes, { flag: 'wx' });
	return files.map((file) => ({ name: file.name, sha256: sha256Bytes(file.bytes) }));
}

export function assertCompleteTlcOutput(output) {
	const report = parseStateReport(output);
	if (
		!/^Model checking completed\. No error has been found\.$/mu.test(output) ||
		!/^Finished in .+$/mu.test(output) ||
		/^Error:/mu.test(output) ||
		report.statesLeft !== 0 ||
		!Number.isSafeInteger(report.generatedStates) ||
		report.generatedStates < 1 ||
		!Number.isSafeInteger(report.distinctStates) ||
		report.distinctStates < 1
	) {
		throw new Error('TLC output does not prove a complete successful state search');
	}
	return report;
}
