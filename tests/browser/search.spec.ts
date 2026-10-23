import { expect, test, type Page } from '@playwright/test';

async function open(page: Page, ruleset = 'chess') {
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Your game' })).toBeVisible();
	if (ruleset !== 'chess') await page.getByLabel('Game variant').selectOption(ruleset);
	if (ruleset === 'go') {
		await page.getByText('Go rules & scoring', { exact: true }).click();
		await page.getByRole('combobox', { name: 'Board', exact: true }).selectOption('9');
		await page.getByRole('button', { name: 'New game', exact: true }).last().click();
	}
}
async function humanMove(page: Page) {
	const origin = page.locator('.board-hit-square.movable-origin').first();
	if (await origin.count()) await origin.click();
	await page.locator('.board-hit-square.legal-target').first().click();
}
async function analysis(page: Page) {
	await page.getByRole('button', { name: 'Analysis', exact: true }).click();
	await expect(page.getByLabel('Best continuations').locator('button').first()).toBeVisible({
		timeout: 30_000
	});
	await expect(page.getByLabel('Position analysis')).toContainText(/depth [1-9]/, {
		timeout: 30_000
	});
}

for (const ruleset of ['chess', 'checkers', 'checkers-international', 'go']) {
	test(`${ruleset}: analysis, arrows and settings use the shared engine`, async ({ page }) => {
		const errors: string[] = [];
		page.on('pageerror', (error) => errors.push(error.message));
		await open(page, ruleset);
		await analysis(page);
		await expect(page.getByLabel('Evaluation bar')).toBeVisible();
		await expect(page.getByLabel('Suggested moves')).toBeVisible();
		await expect(page.locator('.candidate-arrow')).toHaveCount(3, { timeout: 30000 });
		await page.getByLabel('Best continuations').locator('button').nth(1).click();
		await expect(page.getByLabel('Best continuations').locator('button').nth(1)).toHaveAttribute(
			'aria-pressed',
			'true'
		);
		await page.getByLabel('Engine settings').click();
		await page.getByLabel('Best-move arrows').uncheck();
		await expect(page.getByLabel('Suggested moves')).toHaveCount(0);
		await page.getByRole('checkbox', { name: 'Evaluation bar', exact: true }).uncheck();
		await expect(page.locator('.evaluation-bar')).toHaveCount(0);
		await page.getByLabel('Best lines').selectOption('1');
		await page.getByLabel('Best-move arrows').check();
		await expect(page.locator('.candidate-arrow')).toHaveCount(1, { timeout: 30000 });
		await page.getByLabel('Pause search').click();
		await expect(page.getByLabel('Resume search')).toBeVisible();
		await page.getByRole('button', { name: 'Play', exact: true }).click();
		await expect(page.getByLabel('Position analysis')).toHaveCount(0);
		await expect(page.getByLabel('Suggested moves')).toHaveCount(0);
		await expect(page.locator('.evaluation-bar')).toHaveCount(0);
		await expect(page.locator('.move-assessment')).toHaveCount(0);
		expect(errors).toEqual([]);
	});
	test(`${ruleset}: computer moves automatically and replies, with no hints in Play`, async ({
		page
	}) => {
		await open(page, ruleset);
		await page.getByLabel('Engine settings').click();
		await page.getByLabel('Thinking time').selectOption('1000');
		await page.getByLabel('Your side').selectOption('2');
		await expect(page.locator('.move-row button')).toHaveCount(1, { timeout: 30_000 });
		await expect(page.getByLabel('Best continuations')).toHaveCount(0);
		await expect(page.locator('.evaluation-bar')).toHaveCount(0);
		await humanMove(page);
		await expect(page.locator('.move-row button')).toHaveCount(3, { timeout: 30_000 });
		await expect(page.locator('.board-status')).toContainText(
			ruleset === 'go' ? /white/i : /black/i
		);
		await page.getByRole('button', { name: 'Undo', exact: true }).click();
		await expect(page.locator('.move-row button.current')).toHaveText(/.+/);
		await expect(page.getByLabel('Computer opponent')).toContainText('Computer paused');
		await page.getByRole('button', { name: 'Redo', exact: true }).click();
		await expect(page.locator('.move-row button.current')).toHaveCount(1);
	});
}

test('reported midgame keeps deepening and mode changes cancel stale work', async ({ page }) => {
	await open(page);
	await page.getByText('Game · PGN', { exact: true }).click();
	await page
		.getByLabel('Load PGN', { exact: true })
		.fill('1. d4 Nc6 2. Nc3 Nf6 3. Nf3 a5 4. Bf4 h5 5. a4 b6 6. h4 d6 7. b3 Bf5 *');
	await page.getByRole('button', { name: 'Load PGN', exact: true }).click();
	await analysis(page);
	await expect(page.getByLabel('Position analysis')).toContainText(/depth [2-9]/, {
		timeout: 30_000
	});
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await expect(page.getByLabel('Suggested moves')).toHaveCount(0);
	await page.getByLabel('Game variant').selectOption('checkers');
	await expect(page.getByRole('heading', { name: 'Russian draughts', exact: true })).toBeVisible();
	await expect(page.locator('.move-row button')).toHaveCount(0);
});

test('mate is found and computer applies it automatically', async ({ page }) => {
	await open(page);
	await page.getByText('Position · FEN', { exact: true }).click();
	await page.getByLabel('Load FEN', { exact: true }).fill('7k/8/5KQ1/8/8/8/8/8 w - - 0 1');
	await page.getByRole('button', { name: 'Load FEN', exact: true }).click();
	await analysis(page);
	await expect(page.getByLabel('Position evaluation')).toContainText('M1');
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await page.getByLabel('Your side').selectOption('2');
	await expect(page.locator('.board-status')).toContainText(/checkmate/i, { timeout: 30_000 });
	await expect(page.getByLabel('Pause search')).toHaveCount(0);
});

test('history navigation, keyboard and layout work on desktop and mobile', async ({
	page
}, testInfo) => {
	await open(page);
	await page.getByRole('combobox', { name: 'Opponent', exact: true }).selectOption('local');
	await humanMove(page);
	await humanMove(page);
	await expect(page.locator('.move-row button')).toHaveCount(2);
	await page.getByRole('button', { name: 'First move', exact: true }).click();
	await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
	await page.getByRole('button', { name: 'Latest move', exact: true }).click();
	await expect(page.getByRole('button', { name: 'Redo', exact: true })).toBeDisabled();
	await page.locator('.board-status').click();
	await page.keyboard.press('ArrowLeft');
	await expect(page.getByRole('button', { name: 'Redo', exact: true })).toBeEnabled();
	await analysis(page);
	await page.getByLabel('Pause search').click();
	const width = await page.evaluate(() => ({
		content: document.documentElement.scrollWidth,
		viewport: window.innerWidth
	}));
	expect(width.content).toBeLessThanOrEqual(width.viewport + 1);
	await page.screenshot({
		path: testInfo.outputPath(`analysis-${testInfo.project.name}.png`),
		fullPage: true
	});
	await page.getByLabel('Engine settings').focus();
	await expect(page.getByLabel('Engine settings')).toBeFocused();
});

for (const ruleset of ['chess', 'checkers', 'checkers-international', 'go']) {
	test(`${ruleset}: board hit squares stay transparent on hover`, async ({ page }) => {
		await open(page, ruleset);
		const square = page.locator('.board-hit-square').first();
		const transparent = { backgroundColor: 'rgba(0, 0, 0, 0)', opacity: '1' };
		await expect(square).toHaveCSS('background-color', transparent.backgroundColor);
		await square.hover();
		await expect(square).toHaveCSS('background-color', transparent.backgroundColor);
		await expect(square).toHaveCSS('opacity', transparent.opacity);
		if (ruleset !== 'go') {
			const occupied = page.locator('.board-hit-square.movable-origin').first();
			await occupied.hover();
			await expect(occupied).toHaveCSS('background-color', transparent.backgroundColor);
			await expect(page.locator('.piece-node').first()).toBeVisible();
		}
	});
}

test('switching sides puts the human below and relabels the computer correctly', async ({
	page
}) => {
	await open(page);
	await page.getByLabel('Your side').selectOption('2');
	await expect(page.locator('.player-strip').first()).toContainText('Monumla computer');
	await expect(page.locator('.player-strip').first()).toContainText('White');
	await expect(page.locator('.player-strip').last()).toContainText('You');
	await expect(page.locator('.player-strip').last()).toContainText('Black');
	await expect(page.locator('.board-frame')).toHaveClass(/orientation-black/);
	await page.getByLabel('Your side').selectOption('1');
	await expect(page.locator('.player-strip').first()).toContainText('Black');
	await expect(page.locator('.player-strip').last()).toContainText('White');
	await expect(page.locator('.board-frame')).not.toHaveClass(/orientation-black/);
});

test('computer avoids the reported queen blunder in the browser', async ({ page }) => {
	await open(page);
	await page.getByText('Game · PGN', { exact: true }).click();
	await page
		.getByLabel('Load PGN', { exact: true })
		.fill('1. e4 e5 2. Nf3 d5 3. Nc3 dxe4 4. Nxe4 *');
	await page.getByRole('button', { name: 'Load PGN', exact: true }).click();
	await expect(page.locator('.move-row button')).toHaveCount(8, { timeout: 30000 });
	await expect(page.locator('.move-row button.current')).not.toContainText('Qxd2+');
});

test('game review assesses previously played moves and hides badges in Play', async ({ page }) => {
	await open(page);
	await page.getByRole('combobox', { name: 'Opponent', exact: true }).selectOption('local');
	await page.getByText('Game · PGN', { exact: true }).click();
	await page
		.getByLabel('Load PGN', { exact: true })
		.fill('1. e4 e5 2. Nf3 d5 3. Nc3 dxe4 4. Nxe4 Qxd2+ *');
	await page.getByRole('button', { name: 'Load PGN', exact: true }).click();
	await page.getByRole('button', { name: 'Analysis', exact: true }).click();
	await expect(page.locator('.move-row button.current .move-assessment')).toHaveText('??', {
		timeout: 30000
	});
	await expect(page.locator('.board-assessment')).toHaveText('??');
	await expect(page.locator('.board-assessment')).toHaveAttribute('title', /Blunder.*depth/);
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await expect(page.locator('.move-assessment')).toHaveCount(0);
	await expect(page.locator('.board-assessment')).toHaveCount(0);
});

test('analysis navigation retains evaluation, restores cached depth and keeps controls fixed', async ({
	page
}) => {
	await open(page);
	await page.getByRole('combobox', { name: 'Opponent', exact: true }).selectOption('local');
	await analysis(page);
	const panel = page.getByLabel('Position analysis');
	await expect(panel).toHaveAttribute('data-analysis-depth', /[2-9]/, { timeout: 30000 });
	const score = await page.getByLabel('Position evaluation').innerText();
	const retainedDepth = Number(await panel.getAttribute('data-analysis-depth'));
	const bar = await page.locator('.evaluation-bar > div').getAttribute('style');
	const controlsTop = () =>
		page
			.locator('.history-controls')
			.evaluate((element) => element.getBoundingClientRect().top + window.scrollY);
	const before = await controlsTop();
	const held: import('@playwright/test').Route[] = [];
	await page.route(/engine\.worker-[^/]+\.js$/, (route) => {
		held.push(route);
	});
	await humanMove(page);
	await expect(panel).toHaveAttribute('data-analysis-pending', 'true');
	await expect(page.getByLabel('Position evaluation')).toHaveText(score);
	await expect(page.locator('.evaluation-bar > div')).toHaveAttribute('style', bar!);
	await expect(page.getByLabel('Suggested moves')).toHaveCount(0);
	const moved = await controlsTop();
	expect(Math.abs(moved - before)).toBeLessThanOrEqual(1);
	await page.getByRole('button', { name: 'Undo', exact: true }).click();
	await expect(panel).toHaveAttribute('data-analysis-depth', String(retainedDepth));
	await expect(page.getByLabel('Position evaluation')).toHaveText(score);
	await expect(panel).toHaveAttribute('data-analysis-pending', 'false');
	const returned = await controlsTop();
	expect(Math.abs(returned - before)).toBeLessThanOrEqual(1);
	await page.unroute(/engine\.worker-[^/]+\.js$/);
	for (const route of held) await route.continue().catch(() => {});
	// A restarted worker must not replace a deeper cached result with depth one.
	await page.evaluate(() => {
		const panel = document.querySelector('[data-analysis-depth]')!;
		(window as unknown as { observedDepths: number[] }).observedDepths = [];
		new MutationObserver(() =>
			(window as unknown as { observedDepths: number[] }).observedDepths.push(
				Number(panel.getAttribute('data-analysis-depth'))
			)
		).observe(panel, { attributes: true, attributeFilter: ['data-analysis-depth'] });
	});
	await expect(panel).toHaveAttribute('data-analysis-depth', /[3-9]/, { timeout: 30000 });
	const depths = await page.evaluate(
		() => (window as unknown as { observedDepths: number[] }).observedDepths
	);
	expect(depths.every((depth) => depth >= retainedDepth)).toBe(true);
});

test('move assessments have readable names, a legend and distinct quality colors', async ({
	page
}) => {
	await open(page);
	await page.getByRole('combobox', { name: 'Opponent', exact: true }).selectOption('local');
	await page.getByText('Game · PGN', { exact: true }).click();
	await page
		.getByLabel('Load PGN', { exact: true })
		.fill('1. e4 e5 2. Nf3 d5 3. Nc3 dxe4 4. Nxe4 Qxd2+ *');
	await page.getByRole('button', { name: 'Load PGN', exact: true }).click();
	await page.getByRole('button', { name: 'Analysis', exact: true }).click();
	await expect(page.getByLabel('Current move assessment')).toContainText('Qxd2+ · Blunder', {
		timeout: 30000
	});
	await expect(page.locator('.move-row button.current .move-badge')).toHaveAttribute(
		'data-quality',
		'blunder'
	);
	await page.getByText('Move symbols', { exact: true }).click();
	const legend = page.locator('.assessment-key');
	await expect(legend).toContainText('Good move');
	await expect(legend).toContainText('Mistake');
	const good = await legend.locator('span').filter({ hasText: 'Good move' }).getAttribute('style');
	const blunder = await legend.locator('span').filter({ hasText: 'Blunder' }).getAttribute('style');
	expect(good).not.toEqual(blunder);
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await expect(page.getByLabel('Current move assessment')).toHaveCount(0);
});
