// Joins the CSS Module class names that apply, skipping the ones a condition turned off.
export function joinClassNames(...names: readonly (string | false | undefined)[]): string {
	return names.filter((name) => typeof name === 'string' && name !== '').join(' ');
}
