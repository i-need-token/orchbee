import { isDeepStrictEqual } from 'node:util';

/** Only the reviewed ATX-heading shape is admitted. Ambiguity keeps whole-file selection. */
export function documentSections(text) {
	const sections = new Map();
	const preamble = [];
	const references = [];
	const order = [];
	let current = preamble;
	let fence;
	let reference;
	for (const line of text.replaceAll('\r\n', '\n').split('\n')) {
		const delimiter = /^ {0,3}(`{3,}|~{3,})(.*)$/u.exec(line);
		if (fence) {
			current.push(line);
			if (
				delimiter &&
				delimiter[1][0] === fence[0] &&
				delimiter[1].length >= fence.length &&
				!delimiter[2].trim()
			)
				fence = undefined;
			continue;
		}
		if (delimiter) {
			fence = delimiter[1];
			current.push(line);
			continue;
		}
		if (/^ {0,3}(?:=+|-+)\s*$/u.test(line) || /^\s+#{1,6}\s/u.test(line)) return undefined;
		if (
			(/^#\s/u.test(line) && current !== preamble) ||
			/<h[1-6]\b/iu.test(line) ||
			(reference && /^\s+\S/u.test(line))
		)
			return undefined;
		const heading = /^## (\S.*?)\s*$/u.exec(line);
		if (heading) {
			if (/#+$/u.test(heading[1]) || sections.has(heading[1])) return undefined;
			current = [];
			sections.set(heading[1], current);
			order.push(heading[1]);
		} else if (/^##(?:\s|$)/u.test(line)) return undefined;
		reference = /^ {0,3}\[[^\]]+\]:/u.test(line);
		if (reference) {
			if (!/^ {0,3}\[[^\]]+\]:\s*\S/u.test(line)) return undefined;
			references.push(line);
		}
		current.push(line);
	}
	if (fence) return undefined;
	return { sections, preamble: preamble.join('\n'), references: references.join('\n'), order };
}

export function validateDocumentScope(scope, text) {
	const parsed = documentSections(text);
	const headings = [...scope.relevantHeadings, ...scope.unrelatedHeadings];
	if (
		!parsed ||
		new Set(headings).size !== headings.length ||
		headings.length !== parsed.order.length ||
		parsed.order.some((heading) => !headings.includes(heading))
	) {
		throw new Error(`${scope.path} must classify every supported H2 heading exactly once`);
	}
	return parsed;
}

export function documentScopeChanged(scope, before, after) {
	try {
		const previous = validateDocumentScope(scope, before);
		const current = validateDocumentScope(scope, after);
		return (
			previous.preamble !== current.preamble ||
			previous.references !== current.references ||
			!isDeepStrictEqual(previous.order, current.order) ||
			scope.relevantHeadings.some(
				(heading) =>
					!isDeepStrictEqual(previous.sections.get(heading), current.sections.get(heading)),
			)
		);
	} catch {
		return true;
	}
}

function workflowInputs(text) {
	const normalized = text.replaceAll('\r\n', '\n');
	if (/\t|\bneeds["']?\s*:|\b(?:needs|jobs)\s*(?:\.|\[)|^---/mu.test(normalized)) return undefined;
	const lines = normalized.split('\n');
	let scalarIndent;
	for (const line of lines) {
		if (!line.trim()) continue;
		const indentation = /^ */u.exec(line)[0].length;
		if (scalarIndent !== undefined && indentation > scalarIndent) continue;
		scalarIndent = undefined;
		if (/(?:^|[\s,[{:])(?:[&*][^\s,[\]{}&*]+|![^=\s,[\]{}]+)/u.test(line)) return undefined;
		if (/:\s*[|>][-+\d]*\s*(?:#.*)?$/u.test(line)) scalarIndent = indentation;
	}
	const jobs = lines.indexOf('jobs:');
	if (jobs < 0 || lines.lastIndexOf('jobs:') !== jobs) return undefined;
	const admitted = new Set(['name', 'on', 'permissions', 'concurrency', 'env', 'defaults', 'jobs']);
	if (
		lines.some(
			(line) =>
				/^[A-Za-z]/u.test(line) &&
				(!/^([a-z-]+):/u.test(line) || !admitted.has(line.slice(0, line.indexOf(':')))),
		)
	)
		return undefined;
	const blocks = new Map();
	let block;
	for (const line of lines.slice(jobs + 1)) {
		const heading = /^  ([a-z][a-z\d-]*):\s*$/u.exec(line);
		if (heading) {
			if (blocks.has(heading[1])) return undefined;
			block = [];
			blocks.set(heading[1], block);
		} else if (line.trim() && (!block || !/^ {4}/u.test(line))) return undefined;
		block?.push(line);
	}
	const tla = blocks.get('tla');
	if (!tla) return undefined;
	return { globals: lines.slice(0, jobs + 1).join('\n'), tla: tla.join('\n').trimEnd() };
}

export function workflowTlaScope(before, after) {
	const previous = workflowInputs(before);
	const current = workflowInputs(after);
	if (!previous || !current) return 'all';
	if (previous.globals !== current.globals) return 'all';
	return isDeepStrictEqual(previous, current) ? 'none' : 'tooling';
}

export function workflowTlaChanged(before, after) {
	return workflowTlaScope(before, after) !== 'none';
}

/** Keep the mandatory real-TLC witness attached to the supported CI tooling route. */
export function validateToolingWorkflow(text) {
	const parsed = workflowInputs(text);
	const selector = [
		'      - name: Select affected TLA+ models',
		'        id: select',
		'        env:',
		"          TLA_BASE: ${{ github.event_name == 'pull_request' && github.event.pull_request.base.sha || github.event.before }}",
		'        run: node scripts/tla/select-changed-models.mjs "$TLA_BASE"',
	].join('\n');
	const modelCheck = [
		'      - name: Check affected TLA+ models',
		"        if: steps.select.outputs.count != '0'",
		'        run: node scripts/tla/run-models.mjs --models "${{ steps.select.outputs.model_ids }}"',
	].join('\n');
	const cacheCheck = [
		'      - name: Check real TLC cache boundaries',
		"        if: steps.select.outputs.count != '0'",
		'        run: node --test scripts/tla/model-cache-cli.test.mjs',
	].join('\n');
	const steps = parsed?.tla
		.split(/\n(?= {6}- )/u)
		.slice(1)
		.map((step) => step.trimEnd());
	if (
		!parsed ||
		[selector, modelCheck, cacheCheck].some((step) => !steps.includes(step)) ||
		/^    ["']?(?:if|continue-on-error)["']?\s*:/mu.test(parsed.tla)
	)
		throw new Error(
			'The supported CI TLA job must run the real TLC cache witnesses for every nonempty selection',
		);
}
