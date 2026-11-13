import { z } from 'zod';

// Localized text the page script swaps in as the hero story and the header change. The static
// page carries it as one JSON attribute, so the script never holds a second copy of the catalogs.
const clientStringsSchema = z.object({
	live: z.object({ working: z.string(), closed: z.string() }),
	closedFor: z.string(),
	scenes: z.array(z.string()),
});

export type ClientStrings = z.infer<typeof clientStringsSchema>;

const CLIENT_STRINGS_ATTRIBUTE = 'data-website-strings';

/** Reads the strings the static page carries, or undefined on a page without demos. */
export function readClientStrings(root: ParentNode): ClientStrings | undefined {
	const source = root
		.querySelector(`[${CLIENT_STRINGS_ATTRIBUTE}]`)
		?.getAttribute(CLIENT_STRINGS_ATTRIBUTE);
	if (source === null || source === undefined) return undefined;
	const parsed = clientStringsSchema.safeParse(JSON.parse(source));
	return parsed.success ? parsed.data : undefined;
}
