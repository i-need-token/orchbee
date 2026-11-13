const SCROLLED_AT = 8;
const SCENE_LINE = 0.45;
const IDS = ['top', 'anywhere', 'agents', 'orchestrate', 'private', 'terminal', 'install'] as const;

// Scroll-driven header: glass once the page moves, a reading progress line, and a chip naming
// the section that has crossed the upper part of the window.
export function activateHeader(document: Document, sceneNames: readonly string[]): void {
	const nav = document.querySelector<HTMLElement>('[data-nav]');
	const progress = document.querySelector<HTMLElement>('[data-progress]');
	const chip = document.querySelector<HTMLAnchorElement>('[data-scene-chip]');
	const number = document.querySelector<HTMLElement>('[data-scene-number]');
	const name = document.querySelector<HTMLElement>('[data-scene-name]');
	const scenes = [...document.querySelectorAll<HTMLElement>('[data-scene]')];
	let frame = 0;
	let shown = -1;

	const measure = (): void => {
		frame = 0;
		const height = window.innerHeight;
		const y = window.scrollY;
		const max = Math.max(1, document.documentElement.scrollHeight - height);
		if (progress !== null) progress.style.transform = `scaleX(${Math.min(1, y / max).toFixed(4)})`;
		const scrolled = y > SCROLLED_AT;
		nav?.toggleAttribute('data-on', scrolled);
		let scene = 0;
		for (const node of scenes)
			if (node.getBoundingClientRect().top < height * SCENE_LINE)
				scene = Number(node.dataset['scene']) || 0;
		const inScene = scrolled && scene >= 1 && scene <= 6;
		const value = inScene ? scene : 0;
		if (value === shown || chip === null) return;
		shown = value;
		chip.toggleAttribute('data-on', inScene);
		chip.setAttribute('aria-hidden', String(!inScene));
		chip.tabIndex = inScene ? 0 : -1;
		if (!inScene) return;
		chip.hash = IDS[scene] ?? 'top';
		if (number !== null) number.textContent = `0${scene}`;
		if (name !== null) name.textContent = sceneNames[scene] ?? '';
	};
	const schedule = (): void => {
		if (frame === 0) frame = window.requestAnimationFrame(measure);
	};
	window.addEventListener('scroll', schedule, { passive: true });
	window.addEventListener('resize', schedule);
	measure();
}
