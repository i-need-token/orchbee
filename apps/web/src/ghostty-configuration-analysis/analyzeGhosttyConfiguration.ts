const SOURCE_CHARACTER_LIMIT = 262_144;
const SOURCE_LINE_LIMIT = 4096;
const SOURCE_LINE_CHARACTER_LIMIT = 4096;

type GhosttyConfigurationDisposition = 'ignored' | 'invalid' | 'manual' | 'supported';

export type GhosttyConfigurationFindingReason =
	| 'cell-opacity-not-supported'
	| 'color-requires-review'
	| 'config-file-not-loaded'
	| 'font-details-require-review'
	| 'font-family-fallback-list'
	| 'font-family-not-representable'
	| 'font-size-not-representable'
	| 'invalid-value'
	| 'keybind-requires-review'
	| 'lossless-target'
	| 'malformed-line'
	| 'not-in-whitelist'
	| 'opacity-not-representable'
	| 'reset-to-default'
	| 'superseded'
	| 'theme-requires-review'
	| 'window-theme-requires-review'
	| 'working-directory-requires-review';

export type GhosttyConfigurationMigrationTarget =
	| { readonly setting: 'background-opacity'; readonly value: number }
	| { readonly setting: 'custom-background'; readonly value: `#${string}` }
	| { readonly setting: 'custom-foreground'; readonly value: `#${string}` }
	| { readonly setting: 'font-family'; readonly value: string }
	| { readonly setting: 'font-size'; readonly value: number };

interface GhosttyConfigurationAnalysisFindingBase {
	readonly line: number;
	readonly source: string;
}

interface GhosttyConfigurationSupportedFinding extends GhosttyConfigurationAnalysisFindingBase {
	readonly disposition: 'supported';
	readonly reason: 'lossless-target';
	readonly target: GhosttyConfigurationMigrationTarget;
}

interface GhosttyConfigurationUnmappedFinding extends GhosttyConfigurationAnalysisFindingBase {
	readonly disposition: Exclude<GhosttyConfigurationDisposition, 'supported'>;
	readonly reason: Exclude<GhosttyConfigurationFindingReason, 'lossless-target'>;
}

export type GhosttyConfigurationAnalysisFinding =
	| GhosttyConfigurationSupportedFinding
	| GhosttyConfigurationUnmappedFinding;

interface GhosttyConfigurationAnalysisCounts {
	readonly ignored: number;
	readonly invalid: number;
	readonly manual: number;
	readonly supported: number;
}

interface GhosttyConfigurationAnalysisReport {
	readonly counts: GhosttyConfigurationAnalysisCounts;
	readonly findings: readonly GhosttyConfigurationAnalysisFinding[];
	readonly kind: 'report';
}

type GhosttyConfigurationAnalysisRejection =
	| {
			readonly kind: 'rejected';
			readonly limit: number;
			readonly line: number;
			readonly reason: 'line-too-long';
	  }
	| {
			readonly kind: 'rejected';
			readonly limit: number;
			readonly line: null;
			readonly reason: 'source-too-large' | 'too-many-lines';
	  };

export type GhosttyConfigurationAnalysis =
	| GhosttyConfigurationAnalysisRejection
	| GhosttyConfigurationAnalysisReport;

interface Assignment {
	readonly key: string;
	readonly line: number;
	readonly source: string;
	readonly value: string;
}

interface CandidateFinding {
	readonly accepted: boolean;
	readonly finding: GhosttyConfigurationAnalysisFinding;
}

export function analyzeGhosttyConfiguration(source: string): GhosttyConfigurationAnalysis {
	if (source.length > SOURCE_CHARACTER_LIMIT) {
		return sourceRejection('source-too-large', SOURCE_CHARACTER_LIMIT);
	}

	const lines = source.split('\n');
	if (lines.length > SOURCE_LINE_LIMIT) {
		return sourceRejection('too-many-lines', SOURCE_LINE_LIMIT);
	}

	const assignments: Assignment[] = [];
	const findings = new Map<number, GhosttyConfigurationAnalysisFinding>();
	for (const [lineIndex, rawLine] of lines.entries()) {
		const line = lineIndex + 1;
		if (rawLine.length > SOURCE_LINE_CHARACTER_LIMIT) {
			return lineRejection(line, SOURCE_LINE_CHARACTER_LIMIT);
		}

		const trimmed = trimGhosttyWhitespace(rawLine, true);
		if (trimmed.length === 0 || trimmed.startsWith('#')) continue;
		const separator = trimmed.indexOf('=');
		if (separator === -1) {
			findings.set(line, unmappedFinding(line, trimmed, 'invalid', 'malformed-line'));
			continue;
		}

		const key = trimGhosttyWhitespace(trimmed.slice(0, separator), false);
		if (key.length === 0) {
			findings.set(line, unmappedFinding(line, trimmed, 'invalid', 'malformed-line'));
			continue;
		}
		let value = trimGhosttyWhitespace(trimmed.slice(separator + 1), false);
		if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) {
			value = value.slice(1, -1);
		}
		assignments.push(Object.freeze({ key, line, source: trimmed, value }));
	}

	const opacityCells = classifyOpacityCells(assignments, findings);
	classifyScalarAssignments(assignments, findings, 'font-size', classifyFontSize);
	classifyScalarAssignments(assignments, findings, 'foreground', (assignment) =>
		classifyColor(assignment, 'custom-foreground'),
	);
	classifyScalarAssignments(assignments, findings, 'background', (assignment) =>
		classifyColor(assignment, 'custom-background'),
	);
	classifyScalarAssignments(assignments, findings, 'background-opacity', (assignment) =>
		classifyBackgroundOpacity(assignment, opacityCells),
	);
	classifyFontFamilies(assignments, findings);

	for (const assignment of assignments) {
		if (findings.has(assignment.line)) continue;
		findings.set(assignment.line, classifyNonWhitelistAssignment(assignment));
	}

	const orderedFindings = Object.freeze(
		[...findings.values()].sort((left, right) => left.line - right.line),
	);
	const counts: GhosttyConfigurationAnalysisCounts = Object.freeze({
		ignored: orderedFindings.filter(({ disposition }) => disposition === 'ignored').length,
		invalid: orderedFindings.filter(({ disposition }) => disposition === 'invalid').length,
		manual: orderedFindings.filter(({ disposition }) => disposition === 'manual').length,
		supported: orderedFindings.filter(({ disposition }) => disposition === 'supported').length,
	});
	return Object.freeze({ counts, findings: orderedFindings, kind: 'report' });
}

function sourceRejection(
	reason: 'source-too-large' | 'too-many-lines',
	limit: number,
): GhosttyConfigurationAnalysisRejection {
	return Object.freeze({ kind: 'rejected', limit, line: null, reason });
}

function lineRejection(line: number, limit: number): GhosttyConfigurationAnalysisRejection {
	return Object.freeze({ kind: 'rejected', limit, line, reason: 'line-too-long' });
}

function classifyOpacityCells(
	assignments: readonly Assignment[],
	findings: Map<number, GhosttyConfigurationAnalysisFinding>,
): boolean {
	const occurrences = assignments.filter(({ key }) => key === 'background-opacity-cells');
	const candidates = occurrences.map(
		(assignment): CandidateFinding & { readonly value: boolean } => {
			if (assignment.value.length === 0) {
				return {
					accepted: true,
					finding: unmappedAssignment(assignment, 'ignored', 'reset-to-default'),
					value: false,
				};
			}
			if (assignment.value === 'false') {
				return {
					accepted: true,
					finding: unmappedAssignment(assignment, 'ignored', 'not-in-whitelist'),
					value: false,
				};
			}
			if (assignment.value === 'true') {
				return {
					accepted: true,
					finding: unmappedAssignment(assignment, 'manual', 'cell-opacity-not-supported'),
					value: true,
				};
			}
			return {
				accepted: false,
				finding: unmappedAssignment(assignment, 'invalid', 'invalid-value'),
				value: false,
			};
		},
	);
	const effectiveIndex = findLastAcceptedIndex(candidates);
	for (const [index, candidate] of candidates.entries()) {
		const assignment = occurrences[index];
		if (assignment === undefined) continue;
		findings.set(
			assignment.line,
			candidate.accepted && index !== effectiveIndex
				? unmappedAssignment(assignment, 'ignored', 'superseded')
				: candidate.finding,
		);
	}
	return effectiveIndex === -1 ? false : (candidates[effectiveIndex]?.value ?? false);
}

function classifyScalarAssignments(
	assignments: readonly Assignment[],
	findings: Map<number, GhosttyConfigurationAnalysisFinding>,
	key: string,
	classify: (assignment: Assignment) => CandidateFinding,
): void {
	const occurrences = assignments.filter((assignment) => assignment.key === key);
	const candidates = occurrences.map(classify);
	const effectiveIndex = findLastAcceptedIndex(candidates);
	for (const [index, candidate] of candidates.entries()) {
		const assignment = occurrences[index];
		if (assignment === undefined) continue;
		findings.set(
			assignment.line,
			candidate.accepted && index !== effectiveIndex
				? unmappedAssignment(assignment, 'ignored', 'superseded')
				: candidate.finding,
		);
	}
}

function findLastAcceptedIndex(candidates: readonly CandidateFinding[]): number {
	for (let index = candidates.length - 1; index >= 0; index -= 1) {
		if (candidates[index]?.accepted === true) return index;
	}
	return -1;
}

function classifyFontSize(assignment: Assignment): CandidateFinding {
	if (assignment.value.length === 0) return resetCandidate(assignment);
	const value = parseDecimal(assignment.value);
	if (value === null) return invalidCandidate(assignment);
	if (!Number.isInteger(value) || value < 8 || value > 48) {
		return {
			accepted: true,
			finding: unmappedAssignment(assignment, 'manual', 'font-size-not-representable'),
		};
	}
	return supportedCandidate(assignment, { setting: 'font-size', value });
}

function classifyColor(
	assignment: Assignment,
	setting: 'custom-background' | 'custom-foreground',
): CandidateFinding {
	if (assignment.value.length === 0) return resetCandidate(assignment);
	const value = normalizeHexColor(assignment.value);
	if (value !== null) return supportedCandidate(assignment, { setting, value });
	if (assignment.value.startsWith('#')) return invalidCandidate(assignment);
	return {
		accepted: true,
		finding: unmappedAssignment(assignment, 'manual', 'color-requires-review'),
	};
}

function classifyBackgroundOpacity(
	assignment: Assignment,
	opacityCells: boolean,
): CandidateFinding {
	if (assignment.value.length === 0) return resetCandidate(assignment);
	const value = parseDecimal(assignment.value);
	if (value === null) return invalidCandidate(assignment);
	const percentage = value * 100;
	const roundedPercentage = Math.round(percentage);
	if (value < 0.1 || value > 1 || Math.abs(percentage - roundedPercentage) > 1e-9) {
		return {
			accepted: true,
			finding: unmappedAssignment(assignment, 'manual', 'opacity-not-representable'),
		};
	}
	if (opacityCells) {
		return {
			accepted: true,
			finding: unmappedAssignment(assignment, 'manual', 'cell-opacity-not-supported'),
		};
	}
	return supportedCandidate(assignment, {
		setting: 'background-opacity',
		value: roundedPercentage,
	});
}

function classifyFontFamilies(
	assignments: readonly Assignment[],
	findings: Map<number, GhosttyConfigurationAnalysisFinding>,
): void {
	const occurrences = assignments.filter(({ key }) => key === 'font-family');
	let lastResetIndex = -1;
	for (const [index, assignment] of occurrences.entries()) {
		if (assignment.value.length === 0) lastResetIndex = index;
	}
	const active = occurrences.slice(lastResetIndex + 1);
	for (const [index, assignment] of occurrences.entries()) {
		if (index <= lastResetIndex) {
			findings.set(
				assignment.line,
				unmappedAssignment(
					assignment,
					'ignored',
					index === lastResetIndex ? 'reset-to-default' : 'superseded',
				),
			);
		}
	}
	if (active.length > 1) {
		for (const assignment of active) {
			findings.set(
				assignment.line,
				unmappedAssignment(assignment, 'manual', 'font-family-fallback-list'),
			);
		}
		return;
	}
	const assignment = active[0];
	if (assignment === undefined) return;
	if (!isRepresentableFontFamily(assignment.value)) {
		findings.set(
			assignment.line,
			unmappedAssignment(assignment, 'manual', 'font-family-not-representable'),
		);
		return;
	}
	findings.set(
		assignment.line,
		supportedFinding(assignment, { setting: 'font-family', value: assignment.value }),
	);
}

function classifyNonWhitelistAssignment(
	assignment: Assignment,
): GhosttyConfigurationAnalysisFinding {
	if (assignment.key === 'theme') {
		return unmappedAssignment(assignment, 'manual', 'theme-requires-review');
	}
	if (assignment.key === 'window-theme') {
		return unmappedAssignment(assignment, 'manual', 'window-theme-requires-review');
	}
	if (assignment.key === 'keybind') {
		return unmappedAssignment(assignment, 'manual', 'keybind-requires-review');
	}
	if (assignment.key === 'working-directory') {
		return unmappedAssignment(assignment, 'manual', 'working-directory-requires-review');
	}
	if (assignment.key === 'config-file') {
		return unmappedAssignment(assignment, 'manual', 'config-file-not-loaded');
	}
	if (
		assignment.key.startsWith('font-family-') ||
		assignment.key.startsWith('font-feature') ||
		assignment.key.startsWith('font-style') ||
		assignment.key.startsWith('font-variation') ||
		assignment.key === 'font-codepoint-map'
	) {
		return unmappedAssignment(assignment, 'manual', 'font-details-require-review');
	}
	return unmappedAssignment(assignment, 'ignored', 'not-in-whitelist');
}

function parseDecimal(value: string): number | null {
	if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/u.test(value)) return null;
	const parsed = Number(value);
	return Number.isFinite(parsed) ? parsed : null;
}

function trimGhosttyWhitespace(value: string, includeCarriageReturn: boolean): string {
	let start = 0;
	let end = value.length;
	while (isGhosttyWhitespace(value[start], includeCarriageReturn)) start += 1;
	while (end > start && isGhosttyWhitespace(value[end - 1], includeCarriageReturn)) end -= 1;
	return value.slice(start, end);
}

function isGhosttyWhitespace(
	character: string | undefined,
	includeCarriageReturn: boolean,
): boolean {
	return character === ' ' || character === '\t' || (includeCarriageReturn && character === '\r');
}

function normalizeHexColor(value: string): `#${string}` | null {
	const match = /^#?([\da-fA-F]{3}|[\da-fA-F]{6})$/u.exec(value);
	const digits = match?.[1];
	if (digits === undefined) return null;
	if (digits.length === 6) return `#${digits.toLowerCase()}`;
	return `#${[...digits]
		.map((digit) => `${digit}${digit}`)
		.join('')
		.toLowerCase()}`;
}

function isRepresentableFontFamily(value: string): boolean {
	return (
		value.length > 0 &&
		value.length <= 256 &&
		value.trim() === value &&
		!value.includes(',') &&
		!value.includes(';') &&
		![...value].some((character) => {
			const codePoint = character.codePointAt(0);
			return codePoint !== undefined && codePoint <= 0x1f;
		})
	);
}

function resetCandidate(assignment: Assignment): CandidateFinding {
	return {
		accepted: true,
		finding: unmappedAssignment(assignment, 'ignored', 'reset-to-default'),
	};
}

function invalidCandidate(assignment: Assignment): CandidateFinding {
	return {
		accepted: false,
		finding: unmappedAssignment(assignment, 'invalid', 'invalid-value'),
	};
}

function supportedCandidate(
	assignment: Assignment,
	target: GhosttyConfigurationMigrationTarget,
): CandidateFinding {
	return { accepted: true, finding: supportedFinding(assignment, target) };
}

function supportedFinding(
	assignment: Assignment,
	target: GhosttyConfigurationMigrationTarget,
): GhosttyConfigurationSupportedFinding {
	return Object.freeze({
		disposition: 'supported',
		line: assignment.line,
		reason: 'lossless-target',
		source: assignment.source,
		target: Object.freeze(target),
	});
}

function unmappedAssignment(
	assignment: Assignment,
	disposition: GhosttyConfigurationUnmappedFinding['disposition'],
	reason: GhosttyConfigurationUnmappedFinding['reason'],
): GhosttyConfigurationUnmappedFinding {
	return unmappedFinding(assignment.line, assignment.source, disposition, reason);
}

function unmappedFinding(
	line: number,
	source: string,
	disposition: GhosttyConfigurationUnmappedFinding['disposition'],
	reason: GhosttyConfigurationUnmappedFinding['reason'],
): GhosttyConfigurationUnmappedFinding {
	return Object.freeze({ disposition, line, reason, source });
}
