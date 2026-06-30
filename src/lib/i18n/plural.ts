import type {Locale} from './types';

export type PluralCategory = 'one' | 'few' | 'many' | 'other';

// CLDR cardinal category for an integer count. All app counts are integers, so the decimal
// "other" category never occurs for Russian. See specs/006-ui-localization/research.md Decision 3.
export function pluralCategory(loc: Locale, n: number): PluralCategory {
    const i = Math.abs(Math.trunc(n));
    if (loc === 'ru') {
        const m10 = i % 10;
        const m100 = i % 100;
        if (m10 === 1 && m100 !== 11) return 'one';
        if (m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14)) return 'few';
        return 'many';
    }
    return i === 1 ? 'one' : 'other';
}
