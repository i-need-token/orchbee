// The background swarm: particles ride a waggle-dance lemniscate and leave fading heat, and the
// heat is Bayer-dithered into a low-resolution canvas. The seed is fixed, it draws at about 30 fps
// and pauses while the page is hidden. It sits beside the hero, moves to the right edge as the
// page scrolls and gathers around the bee of the closing section. Under reduced motion it draws
// one settled frame beside the hero.

interface Particle {
	u: number;
	readonly w: number;
	readonly o: number;
	readonly wa: number;
	readonly wf: number;
	readonly wp: number;
}
interface Spark {
	u: number;
	readonly w: number;
	readonly o: number;
	readonly f: number;
	readonly p: number;
}
interface Pollen {
	readonly x: number;
	readonly y: number;
	readonly f: number;
	readonly p: number;
	readonly k: 0 | 1;
}
interface Geometry {
	cx: number;
	cy: number;
	A: number;
	th: number;
	ky: number;
	op: number;
}
interface Surface {
	readonly width: number;
	readonly height: number;
	readonly pixel: number;
	readonly heat: Float32Array;
	readonly context: CanvasRenderingContext2D;
	readonly image: ImageData;
	readonly pixels: Uint32Array;
}
type Mode = 'dark' | 'light';

const SEED = 20261028;
const PARTICLES = 5600;
const FRAME_MS = 32;
const SETTLE_FRAMES = 45;
const MOBILE_BELOW = 760;
const MAX_CELLS = 240000;
const POINTER_RADIUS = 90;
const POINTER_FADE_MS = 3000;
const TAU = 6.2832;
const GEOMETRY_KEYS = ['cx', 'cy', 'A', 'th', 'ky', 'op'] as const;
const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map((value) => value / 16);

const rgba = (r: number, g: number, b: number, a: number): number =>
	((a << 24) | (b << 16) | (g << 8) | r) >>> 0;
const PALETTES: Readonly<Record<Mode, readonly number[]>> = {
	dark: [
		0,
		rgba(92, 50, 8, 210),
		rgba(158, 88, 12, 255),
		rgba(222, 134, 20, 255),
		rgba(248, 186, 38, 255),
		rgba(255, 234, 168, 255),
	],
	light: [
		0,
		rgba(222, 196, 140, 170),
		rgba(204, 152, 64, 255),
		rgba(174, 108, 18, 255),
		rgba(134, 74, 8, 255),
		rgba(86, 46, 6, 255),
	],
};
const SPARKS: Readonly<Record<Mode, number>> = {
	dark: rgba(255, 246, 222, 255),
	light: rgba(120, 66, 6, 255),
};
const POLLEN: Readonly<Record<Mode, readonly [number, number]>> = {
	dark: [rgba(255, 238, 205, 46), rgba(255, 238, 205, 96)],
	light: [rgba(110, 72, 20, 40), rgba(110, 72, 20, 80)],
};

function createRandom(seed: number): () => number {
	let state = seed;
	return () => {
		state = (state + 0x6d2b79f5) | 0;
		let t = Math.imul(state ^ (state >>> 15), 1 | state);
		t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

// A point on a lemniscate of Bernoulli, pushed sideways along its normal by o half-widths.
function lemniscate(u: number, o: number): readonly [number, number] {
	const s = Math.sin(u);
	const c = Math.cos(u);
	const den = 1 + s * s;
	const dx = (-s * (3 - s * s)) / (den * den);
	const dy = ((c * c - s * s) * den - 2 * s * s * c * c) / (den * den);
	const length = Math.hypot(dx, dy) || 1;
	return [c / den - (dy / length) * o, (s * c) / den + (dx / length) * o];
}

const smooth = (value: number): number => value * value * (3 - 2 * value);

export function activateSwarm(document: Document, reducedMotion: boolean): void {
	const canvas = document.querySelector<HTMLCanvasElement>('[data-swarm]');
	if (canvas === null) return;
	const bee = document.querySelector<HTMLElement>('[data-outro-bee]');
	const random = createRandom(SEED);
	const gauss = (): number => {
		let u = 0;
		while (u === 0) u = random();
		return Math.sqrt(-2 * Math.log(u)) * Math.cos(TAU * random());
	};
	const groups = Array.from({ length: 7 }, () => ({ u: random() * TAU, w: 0.12 + random() * 0.1 }));
	const particles: Particle[] = [];
	for (let index = 0; index < PARTICLES; index += 1) {
		const group = index < PARTICLES * 0.3 ? groups[index % 7] : undefined;
		const stray = random() < 0.1;
		particles.push({
			u: group === undefined ? random() * TAU : group.u + gauss() * 0.1,
			w: group === undefined ? 0.11 + random() * 0.07 : group.w * (0.96 + random() * 0.08),
			o: gauss() * (stray ? 0.11 : 0.03),
			wa: 0.004 + random() * 0.01,
			wf: 0.6 + random() * 2.2,
			wp: random() * TAU,
		});
	}
	const sparks: Spark[] = [];
	for (let index = 0; index < 240; index += 1)
		sparks.push({
			u: random() * TAU,
			w: 0.04 + random() * 0.05,
			o: gauss() * 0.2,
			f: 0.4 + random() * 1.4,
			p: random() * TAU,
		});
	const pollen: Pollen[] = [];
	for (let index = 0; index < 560; index += 1)
		pollen.push({
			x: random(),
			y: random(),
			f: 0.15 + random() * 0.6,
			p: random() * TAU,
			k: random() < 0.25 ? 1 : 0,
		});

	let surface: Surface | undefined;
	let pointer: { x: number; y: number; t: number } | undefined;
	let shownOpacity = -1;
	let time = 0;
	let last = 0;
	let dirty = true;

	const geometry = (vw: number, vh: number): Geometry => {
		const mobile = vw < MOBILE_BELOW;
		const hero: Geometry = mobile
			? { cx: vw * 0.5, cy: vh * 0.95, A: vw * 0.92, th: -0.32, ky: 1.4, op: 1 }
			: { cx: vw * 0.82, cy: vh * 0.57, A: Math.min(vw * 0.36, 600), th: -0.64, ky: 1.38, op: 1 };
		if (reducedMotion) return hero;
		const middle: Geometry = mobile
			? { cx: vw * 1.04, cy: vh * 0.5, A: vw * 0.85, th: -1.2, ky: 1.4, op: 0.4 }
			: { cx: vw * 1.02, cy: vh * 0.62, A: Math.min(vw * 0.42, 680), th: -1.2, ky: 1.38, op: 0.9 };
		const k1 = smooth(Math.min(1, Math.max(0, window.scrollY / (vh * 1.2))));
		const result = { ...hero };
		for (const key of GEOMETRY_KEYS) result[key] = hero[key] + (middle[key] - hero[key]) * k1;
		if (bee === null) return result;
		const box = bee.getBoundingClientRect();
		const beeY = box.top + box.height / 2;
		const closeness = 1 - (beeY - vh * 0.5) / (vh * 0.8);
		if (closeness <= 0) return result;
		const k2 = smooth(Math.min(1, closeness));
		const end: Geometry = {
			cx: box.left + box.width / 2,
			cy: beeY,
			A: Math.min(vw * (mobile ? 0.46 : 0.34), 480),
			th: 0,
			ky: mobile ? 1.45 : 1.3,
			op: 1,
		};
		for (const key of GEOMETRY_KEYS) result[key] += (end[key] - result[key]) * k2;
		return result;
	};

	const prepare = (vw: number, vh: number): Surface | undefined => {
		let pixel = vw < MOBILE_BELOW ? 2 : 3;
		while ((vw / pixel) * (vh / pixel) > MAX_CELLS) pixel += 1;
		const width = Math.ceil(vw / pixel);
		const height = Math.ceil(vh / pixel);
		if (
			surface !== undefined &&
			surface.width === width &&
			surface.height === height &&
			surface.pixel === pixel
		)
			return surface;
		canvas.width = width;
		canvas.height = height;
		canvas.style.height = `${vh}px`;
		const context = canvas.getContext('2d');
		if (context === null) return undefined;
		const image = context.createImageData(width, height);
		surface = {
			width,
			height,
			pixel,
			heat: new Float32Array(width * height),
			context,
			image,
			pixels: new Uint32Array(image.data.buffer),
		};
		return surface;
	};

	const frame = (t: number, dt: number, draw: boolean): void => {
		const vw = canvas.clientWidth || window.innerWidth;
		const vh = Math.min(window.innerHeight, Math.max(vw * 1.1, 1000));
		const target = prepare(vw, vh);
		if (target === undefined) return;
		const { width: W, height: H, pixel, heat, pixels } = target;
		const g = geometry(vw, vh);
		const A = g.A / pixel;
		const cx = g.cx / pixel;
		const cy = g.cy / pixel;
		const ct = Math.cos(g.th);
		const st = Math.sin(g.th);
		const decay = 0.7 ** (dt * 30);
		for (let index = 0; index < heat.length; index += 1) heat[index] = (heat[index] ?? 0) * decay;
		const age =
			pointer !== undefined && !reducedMotion
				? (performance.now() - pointer.t) / POINTER_FADE_MS
				: 1;
		const pull = age < 1 ? 1 - age : 0;
		const radius = POINTER_RADIUS / pixel;
		const qx = pointer === undefined ? 0 : pointer.x / pixel;
		const qy = pointer === undefined ? 0 : pointer.y / pixel;
		for (const particle of particles) {
			particle.u += particle.w * dt;
			const [px, rawY] = lemniscate(
				particle.u,
				particle.o + particle.wa * Math.sin(t * particle.wf + particle.wp),
			);
			const py = rawY * g.ky;
			let x = cx + A * (px * ct - py * st);
			let y = cy + A * (px * st + py * ct);
			if (pull > 0) {
				const dx = qx - x;
				const dy = qy - y;
				const distance = Math.hypot(dx, dy);
				if (distance < radius) {
					const k = (1 - distance / radius) * (1 - distance / radius) * 0.5 * pull;
					x += dx * k;
					y += dy * k;
				}
			}
			const X = x | 0;
			const Y = y | 0;
			if (X >= 0 && X < W && Y >= 0 && Y < H) heat[Y * W + X] = (heat[Y * W + X] ?? 0) + 1;
		}
		for (const spark of sparks) spark.u += spark.w * dt;
		if (!draw) return;
		const mode: Mode = document.documentElement.dataset['theme'] === 'light' ? 'light' : 'dark';
		const palette = PALETTES[mode];
		for (let y = 0; y < H; y += 1) {
			const by = (y & 3) << 2;
			const row = y * W;
			for (let x = 0; x < W; x += 1) {
				const h = heat[row + x] ?? 0;
				if (h < 0.06) {
					pixels[row + x] = 0;
					continue;
				}
				const level = Math.min(5, (h * 2.4 + (BAYER[by | (x & 3)] ?? 0)) | 0);
				pixels[row + x] = palette[level] ?? 0;
			}
		}
		const dust = POLLEN[mode];
		for (const grain of pollen) {
			if (Math.sin(t * grain.f + grain.p) < 0.2) continue;
			const index = ((grain.y * H) | 0) * W + ((grain.x * W) | 0);
			if (pixels[index] === 0) pixels[index] = dust[grain.k];
		}
		const sparkColor = SPARKS[mode];
		for (const spark of sparks) {
			if (Math.sin(t * spark.f + spark.p) < 0.55) continue;
			const [px, rawY] = lemniscate(spark.u, spark.o);
			const py = rawY * g.ky;
			const X = (cx + A * (px * ct - py * st)) | 0;
			const Y = (cy + A * (px * st + py * ct)) | 0;
			if (X >= 0 && X < W && Y >= 0 && Y < H) pixels[Y * W + X] = sparkColor;
		}
		target.context.putImageData(target.image, 0, 0);
		const opacity = Math.round(g.op * 100) / 100;
		if (opacity !== shownOpacity) {
			shownOpacity = opacity;
			canvas.style.opacity = String(opacity);
		}
	};

	const loop = (ms: number): void => {
		window.requestAnimationFrame(loop);
		if (document.hidden || ms - last < FRAME_MS) return;
		const dt = last === 0 ? 1 / 30 : Math.min(0.1, (ms - last) / 1000);
		last = ms;
		if (reducedMotion) {
			if (!dirty) return;
			dirty = false;
			surface?.heat.fill(0);
			for (let index = 0; index < SETTLE_FRAMES; index += 1)
				frame(index / 30, 1 / 30, index === SETTLE_FRAMES - 1);
			return;
		}
		time += dt;
		frame(time, dt, true);
	};

	window.addEventListener('resize', () => {
		dirty = true;
	});
	window.addEventListener(
		'pointermove',
		(event) => {
			pointer = { x: event.clientX, y: event.clientY, t: performance.now() };
		},
		{ passive: true },
	);
	new MutationObserver(() => {
		dirty = true;
	}).observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
	window.requestAnimationFrame(loop);
}
