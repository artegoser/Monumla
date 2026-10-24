import type { PositionView } from '$lib/wasm/runtime';
import { boardPoint } from './arrow';

export function movementTrack(
	path: PositionView[],
	final: PositionView,
	orientation: 'white' | 'black',
	width: number,
	height: number,
	totalHops = path.length - 1
) {
	const duration = totalHops > 1 ? totalHops * 320 : 200;
	const end = boardPoint(final, orientation, width, height);
	const easing = 'cubic-bezier(0.22, 0.61, 0.36, 1)';
	const frame = (position: PositionView, offset: number, easing: string): Keyframe => {
		const point = boardPoint(position, orientation, width, height);
		return {
			transform: `translate3d(${(point.x - end.x) * width}%, ${(point.y - end.y) * height}%, 0)`,
			offset,
			easing
		};
	};
	const frames = [frame(path[0], 0, easing)];
	for (let i = 1; i < path.length; i++) {
		frames.push(frame(path[i], totalHops > 1 ? ((i - 1) * 320 + 240) / duration : 1, 'linear'));
		if (totalHops > 1) frames.push(frame(path[i], (i * 320) / duration, easing));
	}
	if (frames[frames.length - 1].offset !== 1) frames.push(frame(final, 1, 'linear'));
	return { frames, duration };
}
