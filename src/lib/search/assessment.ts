import type { MoveAssessment } from './types';

const grades: Record<string, { name: string; tone: string; color: string }> = {
	'!!': { name: 'Great move', tone: 'great', color: '#9e9bea' },
	'!': { name: 'Strong move', tone: 'strong', color: '#69b9ca' },
	'✓': { name: 'Best move', tone: 'best', color: '#8cbd67' },
	'·': { name: 'Good move', tone: 'good', color: '#a9bd91' },
	'?!': { name: 'Inaccuracy', tone: 'inaccuracy', color: '#e0c060' },
	'?': { name: 'Mistake', tone: 'mistake', color: '#e6a05c' },
	'??': { name: 'Blunder', tone: 'blunder', color: '#e47768' }
};
export function describeAssessment(value?: MoveAssessment) {
	return value?.symbol && grades[value.symbol]
		? grades[value.symbol]
		: { name: value?.label ?? 'Assessment pending', tone: 'unknown', color: '#a09b92' };
}
export const assessmentLegend = Object.entries(grades).map(([symbol, grade]) => ({
	symbol,
	...grade
}));
