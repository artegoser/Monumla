import { gameHistory } from '../../../server/queries';
export async function load({ locals, url }: import('./$types').PageServerLoadEvent) {
	const page = Math.floor(
		Math.max(1, Math.min(100_000, Number(url.searchParams.get('page')) || 1))
	);
	return {
		history: await gameHistory(
			url.searchParams.get('all') === '1' ? undefined : locals.user?.id,
			page
		),
		own: !!locals.user && url.searchParams.get('all') !== '1'
	};
}
