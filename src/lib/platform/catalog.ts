export const games = [
	{
		id: 'chess',
		name: 'Chess',
		short: 'Chess',
		detail: 'The classic. A world of possibilities.',
		icon: 'chess',
		board: '8 × 8'
	},
	{
		id: 'checkers',
		name: 'Russian checkers',
		short: 'Checkers',
		detail: 'Flying kings. Compulsory captures.',
		icon: 'checkers',
		board: '8 × 8'
	},
	{
		id: 'checkers-international',
		name: 'International checkers',
		short: 'International',
		detail: 'A bigger board. A deeper game.',
		icon: 'checkers',
		board: '10 × 10'
	},
	{
		id: 'go',
		name: 'Go',
		short: 'Go',
		detail: 'Simple rules. Boundless depth.',
		icon: 'go',
		board: '19 × 19'
	},
	{
		id: 'battleship',
		name: 'Battleship',
		short: 'Battleship',
		detail: 'Hide your fleet. Read your opponent.',
		icon: 'battleship',
		board: '10 × 10'
	},
	{
		id: 'tictactoe',
		name: 'Tic-tac-toe',
		short: 'Tic-tac-toe',
		detail: 'Three in a row. Every move matters.',
		icon: 'tictactoe',
		board: '3 × 3'
	}
] as const;

export type Ruleset = (typeof games)[number]['id'];
export const categories = ['bullet', 'blitz', 'rapid', 'classical'] as const;
export const timeControls = [
	{ seconds: 60, increment: 0, label: '1 + 0', category: 'bullet' },
	{ seconds: 120, increment: 1, label: '2 + 1', category: 'bullet' },
	{ seconds: 180, increment: 0, label: '3 + 0', category: 'blitz' },
	{ seconds: 180, increment: 2, label: '3 + 2', category: 'blitz' },
	{ seconds: 300, increment: 0, label: '5 + 0', category: 'blitz' },
	{ seconds: 300, increment: 3, label: '5 + 3', category: 'blitz' },
	{ seconds: 600, increment: 0, label: '10 + 0', category: 'rapid' },
	{ seconds: 900, increment: 10, label: '15 + 10', category: 'rapid' },
	{ seconds: 1800, increment: 0, label: '30 + 0', category: 'classical' }
] as const;

export function categoryFor(seconds: number, increment: number) {
	const estimated = seconds + 40 * increment;
	return estimated < 180
		? 'bullet'
		: estimated < 480
			? 'blitz'
			: estimated < 1500
				? 'rapid'
				: 'classical';
}

export function gameName(id: string) {
	return games.find((game) => game.id === id)?.name ?? id;
}

export function timeLabel(seconds: number, increment: number) {
	return `${seconds / 60} + ${increment}`;
}

export function resultExplanation(reason: string | null | undefined) {
	if (!reason) return 'Game finished';
	const labels: Record<string, string> = {
		'tictactoe.line': 'Three in a row',
		'tictactoe.draw': 'Board filled',
		'battleship.fleet_sunk': 'Fleet sunk',
		'go.scored': 'Scoring complete'
	};
	if (labels[reason]) return labels[reason];
	const readable = reason
		.replace(/^(chess|checkers|go|battleship|tictactoe)\./, '')
		.replace(/[._]+/g, ' ');
	return readable.charAt(0).toUpperCase() + readable.slice(1);
}

export const countryCodes =
	'AD AE AF AG AI AL AM AO AQ AR AS AT AU AW AX AZ BA BB BD BE BF BG BH BI BJ BL BM BN BO BQ BR BS BT BV BW BY BZ CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ DE DJ DK DM DO DZ EC EE EG EH ER ES ET FI FJ FK FM FO FR GA GB GD GE GF GG GH GI GL GM GN GP GQ GR GS GT GU GW GY HK HM HN HR HT HU ID IE IL IM IN IO IQ IR IS IT JE JM JO JP KE KG KH KI KM KN KP KR KW KY KZ LA LB LC LI LK LR LS LT LU LV LY MA MC MD ME MF MG MH MK ML MM MN MO MP MQ MR MS MT MU MV MW MX MY MZ NA NC NE NF NG NI NL NO NP NR NU NZ OM PA PE PF PG PH PK PL PM PN PR PS PT PW PY QA RE RO RS RU RW SA SB SC SD SE SG SH SI SJ SK SL SM SN SO SR SS ST SV SX SY SZ TC TD TF TG TH TJ TK TL TM TN TO TR TT TV TW TZ UA UG UM US UY UZ VA VC VE VG VI VN VU WF WS YE YT ZA ZM ZW'.split(
		' '
	);
const countryNames = new Intl.DisplayNames(['en'], { type: 'region' });
export const countries = countryCodes
	.map((code) => ({ code, name: countryNames.of(code) ?? code }))
	.sort((a, b) => a.name.localeCompare(b.name));
export function countryName(code: string) {
	return code ? (countryNames.of(code) ?? code) : 'Not specified';
}

export interface PublicUser {
	id: string;
	username: string;
	country: string;
	bio: string;
	createdAt: string;
}
