import { expect, test } from '@playwright/test';

test('Battleship supports manual placement, rotation, removal and automatic setup', async ({
	page
}) => {
	await page.goto('/');
	await page.getByLabel('Game variant').selectOption('battleship');
	await expect(page.getByRole('heading', { name: 'Battleship', exact: true })).toBeVisible();
	await expect(page.locator('.sea-cell')).toHaveCount(200);
	await page.getByRole('button', { name: 'Your fleet A1', exact: true }).click();
	await expect(page.locator('.sea-cell.hull')).toHaveCount(4);
	await page.getByRole('button', { name: 'Your fleet A1', exact: true }).click();
	await expect(page.locator('.sea-cell.hull')).toHaveCount(0);
	await page.getByLabel('Rotate ship').click();
	await page.getByRole('button', { name: 'Your fleet B1', exact: true }).click();
	await expect(page.locator('.sea-cell.hull')).toHaveCount(4);
	await expect(page.locator('.sea-cell.hull[data-cell="31"]')).toHaveCount(1);
	await page.getByRole('button', { name: 'Auto arrange', exact: true }).click();
	await expect(page.locator('.sea-cell.hull')).toHaveCount(20);
	await page.getByRole('button', { name: 'Ready', exact: true }).click();
	await expect(page.locator('.placement-status')).toHaveText('Player 1 to fire', {
		timeout: 15000
	});
	await expect(page.locator('.setup-actions')).toHaveCount(0);
	await expect(page.locator('.sea-cell.hull[data-ocean="2"]')).toHaveCount(0);
	const target = page.locator('.sea-cell[data-ocean="2"]:enabled').first();
	await target.click();
	await expect(page.locator('.logbook')).toContainText(/A1 · (hit|miss|sunk)/);
	await expect(page.locator('.engine-error')).toHaveCount(0);
});

test('local Battleship covers the fleet before handing over the device', async ({ page }) => {
	await page.goto('/');
	await page.getByLabel('Game variant').selectOption('battleship');
	await page.getByRole('combobox', { name: /^Opponent/ }).selectOption('local');
	await page.getByRole('button', { name: 'Auto arrange', exact: true }).click();
	await page.getByRole('button', { name: 'Ready', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Pass to Player 2' })).toBeVisible();
	await expect(page.locator('.ocean-grid')).toHaveCount(0);
	await page.getByRole('button', { name: 'I’m Player 2 · show my board' }).click();
	await expect(page.locator('.sea-cell.hull')).toHaveCount(0);
	await page.getByRole('button', { name: 'Auto arrange', exact: true }).click();
	await page.getByRole('button', { name: 'Ready', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Pass to Player 1' })).toBeVisible();
});

test('Battleship analysis uses sampled public information and play hides hints', async ({
	page
}) => {
	await page.goto('/');
	await page.getByLabel('Game variant').selectOption('battleship');
	await page.getByRole('button', { name: 'Auto arrange', exact: true }).click();
	await page.getByRole('button', { name: 'Ready', exact: true }).click();
	await expect(page.locator('.placement-status')).toHaveText('Player 1 to fire', {
		timeout: 15000
	});
	await page.getByRole('button', { name: 'Analysis', exact: true }).click();
	await expect(page.locator('.analysis-summary')).toContainText('sampled layouts', {
		timeout: 15000
	});
	await expect(page.locator('.best-lines button')).toHaveCount(3);
	await expect(page.locator('.shot-candidate')).toHaveCount(3);
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await expect(page.locator('.shot-candidate')).toHaveCount(0);
});

test('tic-tac-toe supports rectangular presets, custom boards and meaningful history', async ({
	page
}) => {
	await page.goto('/');
	await page.getByLabel('Game variant').selectOption('tictactoe');
	await page.getByRole('combobox', { name: /^Opponent/ }).selectOption('local');
	await expect(page.locator('.alignment-cell')).toHaveCount(9);
	for (const cell of ['A1', 'A2', 'B1', 'B2', 'C1']) {
		await page.getByRole('button', { name: `${cell} · Empty`, exact: true }).click();
	}
	await expect(page.locator('.placement-status')).toHaveText('Player 1 wins');
	await expect(page.locator('.logbook')).toContainText('C1');
	await page.getByLabel('Previous move', { exact: true }).click();
	await expect(page.locator('.placement-status')).toContainText('Cross to move');
	await page
		.getByRole('combobox', { name: 'Tic-tac-toe board', exact: true })
		.selectOption('7,5,5');
	await expect(page.locator('.alignment-cell')).toHaveCount(35);
	await page.getByRole('button', { name: 'Custom board', exact: true }).click();
	await page.getByLabel('Columns', { exact: true }).fill('6');
	await page.getByLabel('Rows', { exact: true }).fill('4');
	await page.getByLabel('Connect', { exact: true }).fill('4');
	await page.getByRole('button', { name: 'Create board', exact: true }).click();
	await expect(page.locator('.alignment-cell')).toHaveCount(24);
});

test('tic-tac-toe computer plays automatically and analysis returns three candidates', async ({
	page
}) => {
	await page.goto('/');
	await page.getByLabel('Game variant').selectOption('tictactoe');
	await page.getByRole('button', { name: 'A1 · Empty', exact: true }).click();
	await expect(page.locator('.alignment-cell.circle')).toHaveCount(1, { timeout: 15000 });
	await page.getByRole('button', { name: 'Analysis', exact: true }).click();
	await expect(page.locator('.best-lines button')).toHaveCount(3, { timeout: 15000 });
	await expect(page.locator('.candidate-ring')).toHaveCount(3);
});

test('ship rotation works with orientation controls, R and a placed-ship right click', async ({
	page
}) => {
	await page.goto('/');
	await page.getByLabel('Game variant').selectOption('battleship');
	await page.getByRole('button', { name: 'Vertical', exact: true }).click();
	await expect(page.getByRole('button', { name: 'Vertical', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await page.getByRole('button', { name: 'Your fleet A1', exact: true }).hover();
	await expect(page.locator('.sea-cell.ghost[data-cell="30"]')).toHaveCount(1);
	await page.keyboard.press('r');
	await expect(page.locator('.sea-cell.ghost[data-cell="3"]')).toHaveCount(1);
	await page.getByRole('button', { name: 'Your fleet A1', exact: true }).click();
	await page.getByRole('button', { name: 'Your fleet A1', exact: true }).click({ button: 'right' });
	await expect(page.locator('.sea-cell.hull[data-cell="30"]')).toHaveCount(1);
	await expect(page.locator('.sea-cell.hull[data-cell="3"]')).toHaveCount(0);
	await page.getByRole('button', { name: 'Your fleet A1', exact: true }).hover();
	await page.keyboard.press('r');
	await expect(page.locator('.sea-cell.hull[data-cell="3"]')).toHaveCount(1);
});

test('rotation shortcut uses the physical key in a Russian layout', async ({ page }) => {
	await page.goto('/');
	await page.getByLabel('Game variant').selectOption('battleship');
	await expect(page.getByRole('button', { name: 'Horizontal', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await page.evaluate(() =>
		window.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'к', code: 'KeyR', bubbles: true, cancelable: true })
		)
	);
	await expect(page.getByRole('button', { name: 'Vertical', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
});

test('all alignment presets have visible horizontal and vertical lines and square cells', async ({
	page
}) => {
	await page.goto('/');
	await page.getByLabel('Game variant').selectOption('tictactoe');
	for (const [preset, columns, rows] of [
		['3,3,3', 3, 3],
		['4,4,3', 4, 4],
		['5,5,4', 5, 5],
		['7,5,5', 7, 5]
	] as const) {
		await page
			.getByRole('combobox', { name: 'Tic-tac-toe board', exact: true })
			.selectOption(preset);
		await expect(page.locator('.horizontal-divider')).toHaveCount(rows - 1);
		await expect(page.locator('.vertical-divider')).toHaveCount(columns - 1);
		const geometry = await page.locator('.alignment-board').evaluate((el) => {
			const cells = Array.from(el.querySelectorAll('.alignment-cell')).map((cell) =>
				cell.getBoundingClientRect()
			);
			const board = el.getBoundingClientRect();
			const line = el.querySelector('.horizontal-divider')!;
			return {
				sizes: cells.map((r) => ({ w: r.width, h: r.height })),
				top: cells[0].top,
				bottom: cells[cells.length - 1].bottom,
				boardBottom: board.bottom,
				stroke: getComputedStyle(line).stroke,
				strokeWidth: getComputedStyle(line).strokeWidth
			};
		});
		for (const size of geometry.sizes) expect(Math.abs(size.w - size.h)).toBeLessThan(1);
		expect(geometry.bottom).toBeLessThanOrEqual(geometry.boardBottom + 1);
		expect(geometry.stroke).not.toBe('none');
		expect(parseFloat(geometry.strokeWidth)).toBeGreaterThanOrEqual(2);
	}
});

test('alignment review shows named move quality only in analysis', async ({ page }) => {
	await page.goto('/');
	await page.getByLabel('Game variant').selectOption('tictactoe');
	await page.getByRole('combobox', { name: /^Opponent/ }).selectOption('local');
	for (const cell of ['A1', 'A2', 'B1', 'B2', 'C1'])
		await page.getByRole('button', { name: `${cell} · Empty`, exact: true }).click();
	await expect(page.locator('.move-badge')).toHaveCount(0);
	await page.getByRole('button', { name: 'Analysis', exact: true }).click();
	await expect(page.locator('.placement-review .move-badge')).toBeVisible({ timeout: 20000 });
	await expect(page.locator('.placement-move-grade')).toContainText(
		/Great move|Strong move|Best move/,
		{ timeout: 20000 }
	);
	await expect(page.locator('.log-row .move-badge').last()).toBeVisible();
	await expect(page.locator('.alignment-cell .cell-assessment .move-badge')).toBeVisible();
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await expect(page.locator('.move-badge')).toHaveCount(0);
});

test('Battleship review uses a sampled pre-shot estimate and keeps fog intact', async ({
	page
}) => {
	await page.goto('/');
	await page.getByLabel('Game variant').selectOption('battleship');
	await page.getByRole('button', { name: 'Auto arrange', exact: true }).click();
	await page.getByRole('button', { name: 'Ready', exact: true }).click();
	await expect(page.locator('.placement-status')).toHaveText('Player 1 to fire', {
		timeout: 15000
	});
	await page.getByRole('combobox', { name: /^Opponent/ }).selectOption('local');
	await page.getByRole('button', { name: 'I’m Player 1 · show my board' }).click();
	await page.getByRole('button', { name: 'Analysis', exact: true }).click();
	await page.locator('.sea-cell[data-ocean="2"]:enabled').first().click();
	await expect(page.locator('.placement-move-grade')).toContainText('Sampled estimate', {
		timeout: 20000
	});
	await expect(page.locator('.engine-error')).toHaveCount(0);
	await expect(page.locator('.sea-cell.hull')).toHaveCount(20);
	await expect(page.locator('.sea-cell .cell-assessment .move-badge')).toBeVisible();
});

test('fleet setup previews contain the full footprint and orientation without claiming a ranking', async ({
	page
}) => {
	await page.goto('/');
	await page.getByLabel('Game variant').selectOption('battleship');
	await page.getByRole('button', { name: 'Analysis', exact: true }).click();
	await expect(page.getByRole('heading', { name: 'Fleet placement' })).toBeVisible();
	await expect(page.locator('.placement-options')).toContainText('not strategically ranked');
	await page.getByRole('button', { name: 'Preview Place 4-deck ship vertical at A1' }).hover();
	await expect(page.locator('.sea-cell.ghost')).toHaveCount(4);
	await expect(page.locator('.sea-cell.ghost[data-cell="30"]')).toHaveCount(1);
	await expect(page.getByRole('button', { name: 'Vertical', exact: true })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
});
