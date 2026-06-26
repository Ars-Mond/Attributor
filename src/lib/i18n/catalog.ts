import type {Locale, Messages} from './types';
import {en} from './en';
import {ru} from './ru';

// Lookup table by active locale. Each entry is a full Messages implementation (compile-time enforced).
export const catalog: Record<Locale, Messages> = {en, ru};
