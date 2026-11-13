const VISIBLE_SHARE = 0.25;

// Plays each screen recording while a quarter of it is in view and pauses it otherwise. Under
// reduced motion a recording waits on its poster until the visitor presses play; the button
// pauses and resumes any recording.
export function activateRecordings(document: Document, reducedMotion: boolean): void {
	for (const recording of document.querySelectorAll<HTMLElement>('[data-recording]')) {
		const video = recording.querySelector('video');
		const toggle = recording.querySelector<HTMLButtonElement>('[data-recording-toggle]');
		if (video === null || toggle === null) continue;
		video.muted = true;
		let held = reducedMotion;
		let visible = false;
		const show = (): void => {
			const playing = !video.paused;
			toggle.toggleAttribute('data-playing', playing);
			const label = playing ? toggle.dataset['labelPause'] : toggle.dataset['labelPlay'];
			if (label !== undefined) {
				toggle.setAttribute('aria-label', label);
				toggle.title = label;
			}
		};
		const sync = (): void => {
			if (visible && !held) {
				video.play().catch(() => {
					held = true;
					show();
				});
			} else video.pause();
		};
		video.addEventListener('play', show);
		video.addEventListener('pause', show);
		toggle.addEventListener('click', () => {
			held = !video.paused;
			if (held) video.pause();
			else {
				visible = true;
				sync();
			}
		});
		toggle.hidden = false;
		show();
		new IntersectionObserver(
			([entry]) => {
				visible = entry?.isIntersecting ?? false;
				sync();
			},
			{ threshold: VISIBLE_SHARE },
		).observe(video);
	}
}
