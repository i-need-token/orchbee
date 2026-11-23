/**
 * Where each Agent mark sits in its icon box, so every product reads at one size and height whatever
 * margin its artwork carries. Each view box centers the mark's drawn pixels at 81% of the box height;
 * a wide mark stops at 112% of the box width, which the icon draws past its box. Remeasure a mark's
 * entry when its artwork changes.
 */
export const AGENT_MARK_VIEW_BOXES: ReadonlyMap<string, string> = new Map(
	Object.entries({
		amp: '-2.82 -2.77 29.54 29.54',
		antigravity: '-1.63 -1.56 27.26 27.26',
		atomcode: '-1.69 -1.94 27.63 27.63',
		claude: '1.33 1.83 21.33 21.33',
		'claude-code': '1.33 1.83 21.33 21.33',
		cline: '-2.72 -2.77 29.54 29.54',
		codebuddy: '-1.2 -0.93 27.51 27.51',
		codex: '0.92 0.92 22.15 22.15',
		'command-code': '-2.77 -2.77 29.54 29.54',
		copilot: '0.31 0.31 23.38 23.38',
		cursor: '-2.77 -2.77 29.54 29.54',
		deveco: '0.92 0.92 22.15 22.15',
		devin: '-2.58 -2.58 29.17 29.17',
		droid: '-2.77 -2.77 29.54 29.54',
		gemini: '-2.77 -2.77 29.54 29.54',
		'github-copilot': '0.31 0.31 23.38 23.38',
		grok: '-2.18 -2.16 28.37 28.37',
		hermes: '-2.27 -2.77 29.54 29.54',
		jcode: '0.18 0.18 23.63 23.63',
		kilo: '-2.77 -2.77 29.54 29.54',
		kimi: '1.09 -2.31 24.62 24.62',
		kiro: '-2.77 -2.77 29.54 29.54',
		letta: '-2.77 -2.77 29.54 29.54',
		maki: '1.99 2.32 20.06 20.06',
		mastracode: '-2.7 -2.7 29.35 29.35',
		mimo: '1.33 1.63 21.33 21.33',
		'minimax-code': '-0.31 -0.31 24.62 24.62',
		mothx: '3.16 3.23 17.69 17.69',
		muse: '1.33 1.33 21.33 21.33',
		omp: '-7.38 -7.38 78.77 78.77',
		openclaw: '-1.57 -0.89 27.14 27.14',
		opencode: '-0.31 -0.31 24.62 24.62',
		pi: '-1.54 -1.54 27.08 27.08',
		'prime-agent': '-2.77 -2.77 29.54 29.54',
		qodercli: '-3.07 -2.77 29.54 29.54',
		qwen: '-1.54 -1.54 27.08 27.08',
		'qwen-code': '-1.54 -1.54 27.08 27.08',
		reasonix: '-2.77 -2.77 29.54 29.54',
		snow: '-2.75 -2.72 29.05 29.05',
	}),
);
