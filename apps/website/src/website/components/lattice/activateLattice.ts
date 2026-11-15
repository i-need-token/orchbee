import type { ActivationContext } from '../../activation';

const RADIUS = 23;
const STEP_X = RADIUS * 1.5;
const STEP_Y = RADIUS * Math.sqrt(3);
// Cells near the pointer brighten within this distance.
const POINTER_REACH = 260;
const FRAME_INTERVAL_MS = 42;
// Past this scroll position the lattice is out of view and stops repainting.
const PAINT_UNTIL_SCROLL = 900;
const FLASH_CHANCE = 0.3;
const FLASH_DECAY = 0.95;
const STROKE_ALPHA = 0.032;
const POINTER_ALPHA = 0.12;
const FLASH_STROKE_ALPHA = 0.07;
const FLASH_FILL_ALPHA = 0.05;

interface Cell {
	readonly x: number;
	readonly y: number;
	flash: number;
}

// Draws the hexagon lattice: a faint grid that brightens around the pointer while single cells
// light up and fade. Under reduced motion it is drawn once and stays still.
export function activateLattice({ document, reducedMotion }: ActivationContext): void {
	const canvas = document.querySelector<HTMLCanvasElement>('[data-lattice]');
	const context = canvas?.getContext('2d');
	if (canvas === null || context === null || context === undefined) return;
	const view = document.defaultView;
	if (view === null) return;
	let cells: Cell[] = [];
	let width = 0;
	let height = 0;
	let ratio = 1;
	let pointerX = -Infinity;
	let pointerY = -Infinity;
	let last = 0;
	let color = '';

	const size = (): void => {
		ratio = Math.min(2, view.devicePixelRatio || 1);
		width = canvas.clientWidth;
		height = canvas.clientHeight;
		color = view.getComputedStyle(canvas).color;
		if (width === 0) return;
		canvas.width = Math.round(width * ratio);
		canvas.height = Math.round(height * ratio);
		cells = [];
		for (let column = 0, x = 0; x < width + RADIUS; column += 1, x += STEP_X)
			for (let y = column % 2 === 1 ? STEP_Y / 2 : 0; y < height + STEP_Y; y += STEP_Y)
				cells.push({ x, y, flash: 0 });
	};
	const paint = (): void => {
		context.setTransform(ratio, 0, 0, ratio, 0, 0);
		context.clearRect(0, 0, width, height);
		context.lineWidth = 1;
		context.strokeStyle = color;
		context.fillStyle = color;
		for (const cell of cells) {
			const near = Math.max(
				0,
				1 - Math.hypot(cell.x - pointerX, cell.y - pointerY) / POINTER_REACH,
			);
			context.beginPath();
			for (let corner = 0; corner < 6; corner += 1) {
				const angle = (Math.PI / 3) * corner;
				const x = cell.x + RADIUS * Math.cos(angle);
				const y = cell.y + RADIUS * Math.sin(angle);
				if (corner === 0) context.moveTo(x, y);
				else context.lineTo(x, y);
			}
			context.closePath();
			context.globalAlpha =
				STROKE_ALPHA + near * near * POINTER_ALPHA + cell.flash * FLASH_STROKE_ALPHA;
			context.stroke();
			if (cell.flash > 0.02) {
				context.globalAlpha = cell.flash * FLASH_FILL_ALPHA;
				context.fill();
				cell.flash *= FLASH_DECAY;
			}
		}
		context.globalAlpha = 1;
	};
	const frame = (time: number): void => {
		view.requestAnimationFrame(frame);
		if (time - last < FRAME_INTERVAL_MS || view.scrollY > PAINT_UNTIL_SCROLL) return;
		last = time;
		const cell = cells[Math.floor(Math.random() * cells.length)];
		if (cell !== undefined && Math.random() < FLASH_CHANCE) cell.flash = 1;
		paint();
	};

	canvas.parentElement?.addEventListener('pointermove', (event) => {
		const rect = canvas.getBoundingClientRect();
		pointerX = event.clientX - rect.left;
		pointerY = event.clientY - rect.top;
	});
	view.addEventListener('resize', () => {
		size();
		paint();
	});
	size();
	paint();
	if (!reducedMotion) view.requestAnimationFrame(frame);
}
