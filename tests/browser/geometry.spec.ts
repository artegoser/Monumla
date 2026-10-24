import { test, expect } from '@playwright/test';
import { arrowPath, boardPoint } from '../../src/lib/board/arrow';
import { movementTrack } from '../../src/lib/board/movement';

test('flipping projects every endpoint through the center without changing its move', () => {
	for (const position of [
		{ x: 1, y: 0 },
		{ x: 2, y: 2 },
		{ x: 18, y: 0 }
	]) {
		const white = boardPoint(position, 'white', 19, 19);
		const black = boardPoint(position, 'black', 19, 19);
		expect(white.x + black.x).toBeCloseTo(100);
		expect(white.y + black.y).toBeCloseTo(100);
	}
});

test('a 2:1 leap bends and each arrow has one continuous filled outline', () => {
	const leap = arrowPath({ x: 1, y: 0 }, { x: 2, y: 2 }, 'white', 8, 8, 1.5);
	const straight = arrowPath({ x: 0, y: 0 }, { x: 0, y: 2 }, 'white', 8, 8, 1.5);
	expect(leap.split(' L')).toHaveLength(9);
	expect(straight.split(' L')).toHaveLength(7);
	expect(leap).not.toBe(arrowPath({ x: 1, y: 0 }, { x: 2, y: 2 }, 'black', 8, 8, 1.5));
	const tip = boardPoint({ x: 2, y: 2 }, 'white', 8, 8);
	expect(leap).toContain(`L${tip.x},${tip.y} L`);
	expect(leap.endsWith(' Z')).toBe(true);
});

test('multi-jump playback visits each landing with a visible pause in either direction', () => {
	const path = [
		{ x: 1, y: 1 },
		{ x: 3, y: 3 },
		{ x: 5, y: 5 }
	];
	const forward = movementTrack(path, path[2], 'white', 8, 8);
	expect(forward.duration).toBe(640);
	expect(forward.frames.map((frame) => frame.offset)).toEqual([0, 0.375, 0.5, 0.875, 1]);
	expect(forward.frames[0].transform).toBe('translate3d(-400%, 400%, 0)');
	expect(forward.frames[1].transform).toBe('translate3d(-200%, 200%, 0)');
	expect(forward.frames[2].transform).toBe(forward.frames[1].transform);
	expect(forward.frames[4].transform).toBe('translate3d(0%, 0%, 0)');
	const reverse = movementTrack([...path].reverse(), path[0], 'white', 8, 8);
	expect(reverse.duration).toBe(640);
	expect(reverse.frames[1].transform).toBe('translate3d(200%, -200%, 0)');
});
