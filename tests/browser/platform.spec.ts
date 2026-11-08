import { expect, test, type Page } from '@playwright/test';
import { mkdirSync } from 'node:fs';

async function register(page: Page, username: string) {
	await page.goto('/register');
	await page.getByLabel(/^Username/).fill(username);
	await page.getByLabel(/^Email/).fill(`${username}@example.com`);
	await page.getByLabel(/^Password/).fill('a secure browser password');
	await page.getByLabel('Country', { exact: false }).selectOption('JP');
	await page.getByRole('button', { name: 'Create account', exact: true }).click();
	await expect(page).toHaveURL('/');
	await expect(page.locator('.connection-label')).toHaveText('Connected');
}

test('accounts, profiles, friends, challenges, live play, Elo, replay, and computer games', async ({
	page,
	browser
}, testInfo) => {
	test.setTimeout(100_000);
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	const suffix = `${testInfo.project.name}_${Date.now().toString(36).slice(-5)}`;
	const alice = `Alice_${suffix}`,
		bob = `Bob_${suffix}`;
	mkdirSync('.impeccable/review', { recursive: true });
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'Find your next game.' })).toBeVisible();
	await expect(page.locator('.connection-label')).toHaveText('Connected');
	const source = await page.request.get('/source.tar.gz');
	expect(source.ok()).toBe(true);
	expect(source.headers()['content-type']).toContain('application/gzip');
	expect(source.headers()['content-encoding']).toBeUndefined();
	expect([...(await source.body()).subarray(0, 2)]).toEqual([0x1f, 0x8b]);
	await page.screenshot({
		path: `.impeccable/review/${testInfo.project.name}.png`,
		fullPage: true
	});
	expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
	await register(page, alice);
	await page.goto('/settings');
	await page.getByLabel('Country', { exact: false }).selectOption('DE');
	await page.getByLabel('About you').fill('Chess, Go, and finding the next good game.');
	await page.getByRole('button', { name: 'Save profile' }).click();
	await expect(page.getByRole('status')).toContainText('Your profile has been saved.');
	await page.goto(`/u/${alice}`);
	await expect(page.locator('.profile-identity')).toContainText('Germany');
	await expect(page.locator('.profile-bio')).toHaveText(
		'Chess, Go, and finding the next good game.'
	);
	await page.screenshot({
		path: `.impeccable/review/${testInfo.project.name}-profile.png`,
		fullPage: true
	});
	const otherContext = await browser.newContext({
		viewport: page.viewportSize() ?? { width: 1280, height: 720 }
	});
	const other = await otherContext.newPage();
	try {
		await register(other, bob);
		await page.goto('/players');
		await page.getByRole('textbox', { name: 'Search players' }).fill(bob);
		await expect(page.locator('.directory-player')).toHaveCount(1);
		await page.locator('.directory-player').click();
		await page.getByRole('button', { name: 'Add friend', exact: true }).click();
		await other.goto('/friends');
		await other.getByRole('button', { name: 'Accept', exact: true }).click();
		await expect(page.getByRole('button', { name: 'Remove friend', exact: true })).toBeVisible();
		await page.getByRole('link', { name: 'Challenge to a game' }).click();
		await page.getByRole('button', { name: /Tic-tac-toe/ }).click();
		await page.getByRole('button', { name: '3 + 0 blitz', exact: true }).click();
		await page.getByRole('button', { name: 'Send challenge', exact: true }).click();
		await expect(other.locator('.challenge-banner')).toContainText(alice);
		await other
			.locator('.challenge-banner')
			.getByRole('button', { name: 'Accept', exact: true })
			.click();
		await expect(page).toHaveURL(/\/game\//);
		await expect(other).toHaveURL(/\/game\//);
		const first = (await page.locator('.match-spectator').textContent())?.includes('player 1')
			? page
			: other;
		const second = first === page ? other : page;
		for (const [target, square] of [
			[first, 'A1'],
			[second, 'A2'],
			[first, 'B1'],
			[second, 'B2'],
			[first, 'C1']
		] as const) {
			const cell = target.getByRole('button', { name: `${square} · Empty`, exact: true });
			await expect(cell).toBeEnabled();
			await cell.click();
		}
		await expect(page.locator('.match-status')).toContainText('1-0');
		await expect(page.locator('.match-status')).toContainText('Three in a row');
		await expect(page.getByRole('button', { name: 'Flip board', exact: true })).toHaveCount(0);
		await expect(other.locator('.match-status')).toContainText('1-0');
		await page.screenshot({
			path: `.impeccable/review/${testInfo.project.name}-match.png`,
			fullPage: true
		});
		const gameUrl = page.url();
		await page.reload();
		await expect(page.locator('.match-status')).toContainText('1-0');
		await expect(page.locator('.match-status')).toContainText('Three in a row');
		await expect(page.getByRole('button', { name: 'Flip board', exact: true })).toHaveCount(0);
		await page.getByRole('button', { name: 'Starting position', exact: true }).click();
		await expect(page.locator('.alignment-cell svg')).toHaveCount(0);
		await page.getByRole('button', { name: 'Next turn', exact: true }).click();
		await expect(page.locator('.alignment-cell svg')).toHaveCount(1);
		await page.getByRole('button', { name: 'Final position', exact: true }).click();
		await expect(page.locator('.alignment-cell svg')).toHaveCount(5);
		await page.goto('/games');
		await expect(page.locator('.history-entry')).toHaveCount(1);
		await expect(page.locator('.history-entry')).toHaveAttribute('href', new URL(gameUrl).pathname);
		await page.goto('/leaderboard?game=tictactoe&mode=blitz');
		await expect(page.locator('.leaderboard-table')).toContainText(alice);
		await expect(page.locator('.leaderboard-table')).toContainText(bob);
		await expect(page.locator('.leaderboard-table')).toContainText('1520');
		await page.goto('/');
		await page.getByRole('button', { name: /Tic-tac-toe/ }).click();
		await page.getByRole('tab', { name: 'Computer', exact: true }).click();
		await page.getByRole('button', { name: 'Play with the computer' }).click();
		await expect(page).toHaveURL(/\/game\//);
		const initial = page.getByRole('button', { name: 'A1 · Empty', exact: true });
		await expect(initial).toBeEnabled();
		await initial.click();
		await expect(page.locator('.match-notations > button')).toHaveCount(2, { timeout: 15_000 });
		await page.getByRole('button', { name: 'Resign', exact: true }).click();
		await page.getByRole('button', { name: 'Confirm resignation' }).click();
		await expect(page.locator('.match-status')).toContainText('0-1');
		await page.goto('/games');
		await expect(page.locator('.history-entry')).toHaveCount(2);
		await page.goto('/settings');
		await page.getByRole('button', { name: 'Sign out of Monumla' }).click();
		await expect(page.getByRole('link', { name: 'Sign in', exact: true })).toBeVisible();
		await page.goto('/login');
		await page.getByLabel('Username or email').fill(alice);
		await page.getByLabel('Password', { exact: true }).fill('wrong password');
		await page.getByRole('button', { name: 'Sign in', exact: true }).click();
		await expect(page.getByRole('alert')).toContainText('Check your username and password.');
		await page.getByLabel(/^Password/).fill('a secure browser password');
		await page.getByRole('button', { name: 'Sign in', exact: true }).click();
		await expect(page).toHaveURL('/');
		expect(errors).toEqual([]);
	} finally {
		await otherContext.close();
	}
});
