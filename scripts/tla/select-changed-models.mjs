import { appendFile, readFile } from 'node:fs/promises';

import { inventoryPath, selectModelsSince, validateInventory } from './model-inventory.mjs';

const inventory = validateInventory(JSON.parse(await readFile(inventoryPath, 'utf8')));
const reference = process.argv[2];

let models;
if (reference === undefined || reference.length === 0 || /^0+$/u.test(reference)) {
	models = inventory.models;
} else {
	try {
		models = await selectModelsSince(inventory, reference, {
			includeUntracked: process.env.CI !== 'true',
		});
	} catch (error) {
		process.stderr.write(
			`Could not resolve changed TLA+ models; selecting all: ${error instanceof Error ? error.message : String(error)}\n`,
		);
		models = inventory.models;
	}
}

const identifiers = models.map(({ id }) => id).join(',');
process.stdout.write(`${identifiers}\n`);
if (process.env.GITHUB_OUTPUT !== undefined) {
	await appendFile(
		process.env.GITHUB_OUTPUT,
		`count=${models.length}\nmodel_ids=${identifiers}\n`,
		'utf8',
	);
}
