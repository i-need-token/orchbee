export interface TextShapingCell {
	readonly text: string;
	readonly width: number;
	readonly styleKey: string;
	readonly eligible: boolean;
}

export interface TextShapingSpan {
	readonly start: number;
	readonly end: number;
	readonly columns: number;
	readonly text: string;
}

export type TextShapingPlan = readonly (TextShapingSpan | null)[];

const PROGRAMMING_LIGATURES = [
	'<==>',
	'<-->',
	'<!--',
	'||||>',
	'====',
	'!==',
	'===',
	'==>',
	'<==',
	'<=>',
	'<->',
	'<--',
	'<~~',
	'<<=',
	'<<<',
	'-->',
	'->>',
	'-<<',
	'...',
	'..<',
	'..=',
	'/**',
	'///',
	':::',
	'>==',
	'>>=',
	'>>>',
	'<|>',
	'<||',
	'||>',
	'|||',
	'~~>',
	'~>>',
	'~~~',
	'###',
	'***',
	'+++',
	'---',
	'www',
	'!=',
	'==',
	'=>',
	'<=',
	'>=',
	'<>',
	'->',
	'<-',
	'<~',
	'~>',
	'-<',
	'>-',
	'..',
	'//',
	'/*',
	'*/',
	'/=',
	'/>',
	'</',
	'::',
	':=',
	'!!',
	'??',
	'?.',
	'?:',
	'&&',
	'||',
	'|=',
	'|>',
	'<|',
	'++',
	'--',
	'**',
	'##',
	'#{',
	'#[',
	'#(',
	'#!',
] as const;

/** Build bounded multi-cell runs that Canvas can shape without caching arbitrary terminal lines. */
export function buildTextShapingPlan(cells: readonly TextShapingCell[]): TextShapingPlan {
	const plan: (TextShapingSpan | null)[] = Array.from({ length: cells.length }, () => null);
	let column = 0;
	while (column < cells.length) {
		const span = findSpan(cells, column);
		if (!span) {
			column++;
			continue;
		}
		for (let index = span.start; index < span.end; index++) plan[index] = span;
		column = span.end;
	}
	return plan;
}

function findSpan(cells: readonly TextShapingCell[], start: number): TextShapingSpan | null {
	const first = cells[start];
	if (!first?.eligible || first.width !== 1) return null;

	for (const candidate of PROGRAMMING_LIGATURES) {
		if (!matches(cells, start, candidate, first.styleKey)) continue;
		if (extendsRepeatedCharacterRun(cells, start, candidate, first.styleKey)) continue;
		return {
			start,
			end: start + candidate.length,
			columns: candidate.length,
			text: candidate,
		};
	}
	return null;
}

function extendsRepeatedCharacterRun(
	cells: readonly TextShapingCell[],
	start: number,
	candidate: string,
	styleKey: string,
): boolean {
	const character = candidate[0];
	if (!character) return false;
	for (let index = 1; index < candidate.length; index++) {
		if (candidate[index] !== character) return false;
	}
	return (
		matchesRepeatedCharacter(cells[start - 1], character, styleKey) ||
		matchesRepeatedCharacter(cells[start + candidate.length], character, styleKey)
	);
}

function matchesRepeatedCharacter(
	cell: TextShapingCell | undefined,
	character: string,
	styleKey: string,
): boolean {
	return (
		cell?.eligible === true &&
		cell.width === 1 &&
		cell.styleKey === styleKey &&
		cell.text === character
	);
}

function matches(
	cells: readonly TextShapingCell[],
	start: number,
	candidate: string,
	styleKey: string,
): boolean {
	if (start + candidate.length > cells.length) return false;
	for (let offset = 0; offset < candidate.length; offset++) {
		const cell = cells[start + offset];
		if (
			!cell?.eligible ||
			cell.width !== 1 ||
			cell.styleKey !== styleKey ||
			cell.text !== candidate[offset]
		) {
			return false;
		}
	}
	return true;
}
