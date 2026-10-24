import type { PositionView } from '$lib/wasm/runtime';

export function boardPoint(
	p: PositionView,
	orientation: 'white' | 'black',
	width: number,
	height: number
) {
	return {
		x: ((orientation === 'white' ? p.x + 0.5 : width - p.x - 0.5) * 100) / width,
		y: ((orientation === 'white' ? height - p.y - 0.5 : p.y + 0.5) * 100) / height
	};
}

/** One filled outline: shaft and arrowhead cannot overlap or protrude through each other. */
export function arrowPath(
	from: PositionView,
	to: PositionView,
	orientation: 'white' | 'black',
	width: number,
	height: number,
	thickness: number
) {
	const start = boardPoint(from, orientation, width, height);
	const tip = boardPoint(to, orientation, width, height);
	if (from.x === to.x && from.y === to.y) return '';
	const dx = Math.abs(from.x - to.x);
	const dy = Math.abs(from.y - to.y);
	// Route a 2:1 leap along its long axis first. This is display geometry, not a movement rule.
	const bend =
		Math.min(dx, dy) === 1 && Math.max(dx, dy) === 2
			? dx > dy
				? { x: tip.x, y: start.y }
				: { x: start.x, y: tip.y }
			: null;
	const last = bend ?? start;
	const length = Math.hypot(tip.x - last.x, tip.y - last.y);
	const ux = (tip.x - last.x) / length;
	const uy = (tip.y - last.y) / length;
	const nx = -uy;
	const ny = ux;
	const half = thickness / 2;
	const headLength = Math.min(thickness * 2.8, length * 0.7);
	const headHalf = thickness * 1.8;
	const base = { x: tip.x - ux * headLength, y: tip.y - uy * headLength };
	const points: PositionView[] = [];
	let firstNormal = { x: nx, y: ny };
	if (bend) {
		const firstLength = Math.hypot(bend.x - start.x, bend.y - start.y);
		firstNormal = { x: -(bend.y - start.y) / firstLength, y: (bend.x - start.x) / firstLength };
	}
	points.push({ x: start.x + firstNormal.x * half, y: start.y + firstNormal.y * half });
	if (bend)
		points.push({
			x: bend.x + (firstNormal.x + nx) * half,
			y: bend.y + (firstNormal.y + ny) * half
		});
	points.push(
		{ x: base.x + nx * half, y: base.y + ny * half },
		{ x: base.x + nx * headHalf, y: base.y + ny * headHalf },
		tip,
		{ x: base.x - nx * headHalf, y: base.y - ny * headHalf },
		{ x: base.x - nx * half, y: base.y - ny * half }
	);
	if (bend)
		points.push({
			x: bend.x - (firstNormal.x + nx) * half,
			y: bend.y - (firstNormal.y + ny) * half
		});
	points.push({ x: start.x - firstNormal.x * half, y: start.y - firstNormal.y * half });
	return `M${points.map((p) => `${p.x},${p.y}`).join(' L')} A${half},${half} 0 0 1 ${points[0].x},${points[0].y} Z`;
}
