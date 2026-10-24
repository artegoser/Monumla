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
		if (ruleset === 'go') {
			await expect(page.locator('.go-grid-lines line').first()).toHaveCSS(
				'vector-effect',
				'non-scaling-stroke'
			);
			await expect(page.locator('.go-board')).toHaveCSS('background-image', 'none');
		}
		await expect(page.getByLabel('Evaluation bar')).toBeVisible();
		await expect(page.getByLabel('Suggested moves')).toBeVisible();
		await expect(page.locator('.candidate-arrow')).toHaveCount(3, { timeout: 30000 });
		await expect(page.locator('.candidate-arrow text')).toHaveCount(0);
		const firstArrow = page.locator('.candidate-arrow[data-rank="1"]');
		const secondArrow = page.locator('.candidate-arrow[data-rank="2"]');
		const thirdArrow = page.locator('.candidate-arrow[data-rank="3"]');
		expect(Number(await firstArrow.getAttribute('opacity'))).toBeGreaterThan(
			Number(await secondArrow.getAttribute('opacity'))
		);
		expect(Number(await secondArrow.getAttribute('opacity'))).toBeGreaterThan(
			Number(await thirdArrow.getAttribute('opacity'))
		);
		expect(
			await firstArrow
				.locator('path,circle')
				.first()
				.evaluate((el) => el.getAttribute('stroke') ?? el.getAttribute('fill'))
		).not.toEqual(
			await secondArrow
				.locator('path,circle')
				.first()
				.evaluate((el) => el.getAttribute('stroke') ?? el.getAttribute('fill'))
		);
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
	await expect(page.getByRole('heading', { name: 'Russian checkers', exact: true })).toBeVisible();
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
	await page.getByLabel('Pause search').click();
	const score = await page.getByLabel('Position evaluation').innerText();
	const retainedDepth = Number(await panel.getAttribute('data-analysis-depth'));
	const bar = await page.locator('.evaluation-bar > div').getAttribute('style');
	const controlsTop = () =>
		page
			.locator('.history-controls')
			.evaluate((element) => element.getBoundingClientRect().top + window.scrollY);
	const before = await controlsTop();
	await page.evaluate(() => {
		const original = Worker.prototype.postMessage;
		const blocked: Array<{ worker: Worker; message: unknown }> = [];
		Worker.prototype.postMessage = function (message: unknown) {
			const request = message as { settings?: unknown; ply?: number };
			if (request.settings && request.ply == null) blocked.push({ worker: this, message });
			else original.call(this, message);
		};
		(window as unknown as { releaseAnalysis: () => void }).releaseAnalysis = () => {
			Worker.prototype.postMessage = original;
			const last = blocked.at(-1);
			if (last) original.call(last.worker, last.message);
		};
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
	await page.getByLabel('Resume search').click();
	await page.evaluate(() =>
		(window as unknown as { releaseAnalysis: () => void }).releaseAnalysis()
	);
	// Resuming must not replace a deeper cached result with depth one.
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

test('flipping analysis rotates arrows without changing the turn', async ({ page }) => {
	await open(page);
	await analysis(page);
	await page.getByLabel('Pause search').click();
	const arrow = page.locator('.candidate-arrow[data-rank="1"] path').first();
	const before = await arrow.getAttribute('d');
	const status = await page.locator('.board-status').textContent();
	await page.getByRole('button', { name: 'Flip board', exact: true }).click();
	await expect(arrow).not.toHaveAttribute('d', before!);
	await expect(page.locator('.board-status')).toHaveText(status!);
	await expect(page.locator('.move-row button')).toHaveCount(0);
});

test('computer versus computer plays both sides, pauses and keeps draughts notation', async ({
	page
}) => {
	await open(page, 'checkers');
	await page.getByRole('combobox', { name: 'Opponent', exact: true }).selectOption('bots');
	await expect
		.poll(() => page.locator('.move-row button').count(), { timeout: 30000 })
		.toBeGreaterThanOrEqual(2);
	await expect(page.locator('.player-strip').first()).toContainText('Monumla computer');
	await expect(page.locator('.player-strip').last()).toContainText('Monumla computer');
	await expect(page.locator('.move-row button').first()).toContainText(/[a-h][1-8][-x][a-h][1-8]/);
	await page.getByLabel('Pause search').click();
	await expect(page.getByLabel('Computer opponent')).toContainText('Computer paused');
	const count = await page.locator('.move-row button').count();
	await page.waitForTimeout(1200);
	await expect(page.locator('.move-row button')).toHaveCount(count);
	await expect(page.getByLabel('Suggested moves')).toHaveCount(0);
	await page.getByLabel('Resume search').click();
	await expect
		.poll(() => page.locator('.move-row button').count(), { timeout: 30000 })
		.toBeGreaterThan(count);
});

test('a published best move gets its existing assessment without another review search', async ({
	page
}) => {
	await open(page);
	await page.getByRole('combobox', { name: 'Opponent', exact: true }).selectOption('local');
	await analysis(page);
	await expect(page.getByLabel('Position analysis')).toHaveAttribute(
		'data-analysis-depth',
		/[2-9]/,
		{ timeout: 30000 }
	);
	await page.getByLabel('Pause search').click();
	const arrow = page.locator('.candidate-arrow[data-rank="1"] path').first();
	const from = (await arrow.getAttribute('data-from'))!.split(',').map(Number);
	const to = (await arrow.getAttribute('data-to'))!.split(',').map(Number);
	await page.evaluate(() => {
		const original = Worker.prototype.postMessage;
		(window as unknown as { reviewRequests: number }).reviewRequests = 0;
		Worker.prototype.postMessage = function (message: unknown) {
			if ((message as { ply?: number }).ply != null)
				(window as unknown as { reviewRequests: number }).reviewRequests++;
			original.call(this, message);
		};
	});
	const square = (position: number[]) =>
		page
			.locator('.board-hit-square')
			.filter({ hasText: '' })
			.and(
				page.getByRole('button', {
					name: new RegExp(`(?:Square |on )${position[0] + 1},${position[1] + 1}$`)
				})
			);
	await square(from).click();
	await square(to).click();
	await expect(page.getByLabel('Current move assessment')).toContainText(
		/Best move|Strong move|Great move/
	);
	expect(
		await page.evaluate(() => (window as unknown as { reviewRequests: number }).reviewRequests)
	).toBe(0);
});

test('right click marks a circle without filling or hiding the piece', async ({ page }) => {
	await open(page);
	const square = page.locator('.board-hit-square.movable-origin').first();
	await square.click({ button: 'right' });
	const mark = page.locator('.annotation-square');
	await expect(mark).toHaveCSS('background-color', 'rgba(0, 0, 0, 0)');
	await expect(mark).toHaveCount(1);
	const circle = await mark.evaluate((el) => {
		const style = getComputedStyle(el, '::after');
		return { radius: style.borderRadius, border: style.borderTopStyle };
	});
	expect(circle).toEqual({ radius: '50%', border: 'solid' });
	await expect(page.locator('.piece-node').first()).toBeVisible();
});

test('live analysis reports a provisional deeper iteration without promoting its completed depth', async ({
	page
}) => {
	await open(page);
	await page.getByRole('button', { name: 'Analysis', exact: true }).click();
	const panel = page.getByLabel('Position analysis');
	await expect(panel).toHaveAttribute('data-provisional', 'true', { timeout: 30000 });
	await expect
		.poll(async () => {
			const completed = Number(await panel.getAttribute('data-analysis-depth'));
			const searching = Number(await panel.getAttribute('data-searching-depth'));
			return searching > completed;
		})
		.toBe(true);
	await expect(panel.locator('.analysis-summary')).toContainText('Provisional');
});

test('review totals remain fixed when navigating the same game backwards', async ({ page }) => {
	await open(page);
	await page.getByRole('combobox', { name: 'Opponent', exact: true }).selectOption('local');
	for (let i = 0; i < 3; i++) await humanMove(page);
	await analysis(page);
	const review = page.getByLabel('Game review');
	await expect(review).toContainText('/3', { timeout: 30000 });
	await page.getByRole('button', { name: 'Undo', exact: true }).click();
	await expect(review).toContainText('/3');
	await page.getByRole('button', { name: 'Redo', exact: true }).click();
	await expect(review).toContainText('/3');
});

test('shared search can choose a draw claim and the ordinary action finishes the game', async ({
	page
}) => {
	await open(page);
	await page.getByRole('button', { name: 'Analysis', exact: true }).click();
	await page.getByText('Position · FEN', { exact: true }).click();
	await page.getByLabel('Load FEN', { exact: true }).fill('7k/8/8/8/8/8/8/K6q w - - 100 51');
	await page.getByRole('button', { name: 'Load FEN', exact: true }).click();
	await expect(page.locator('.best-lines button').first()).toContainText(
		'Fifty-move draw claimed',
		{ timeout: 30000 }
	);
	await page.getByRole('button', { name: 'Claim fifty-move draw', exact: true }).first().click();
	await expect(page.locator('.board-status')).toContainText('Draw · 50-move rule');
});
