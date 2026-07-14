import { startLoc } from '../utils.js';

const SUPPRESSION_PATTERN = /(?:oxlint-disable|eslint-disable)/;

/** @type {import('eslint').Rule.RuleModule} */
export const noInlineSuppressions = {
	meta: {
		type: 'problem',
		docs: {
			description: 'Disallow inline comments that suppress lint diagnostics.',
		},
		schema: [],
	},
	create(context) {
		return {
			Program() {
				for (const comment of context.sourceCode.getAllComments()) {
					const suppression = comment.value.match(SUPPRESSION_PATTERN)?.[0];
					if (!suppression) continue;
					context.report({
						loc: startLoc(comment),
						message: `Remove inline lint suppression "${suppression}"; fix the code or change the project rule with human approval. See docs/CODESTYLE.md#inline-suppressions.`,
					});
				}
			},
		};
	},
};
