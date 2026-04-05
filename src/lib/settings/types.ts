export type FieldType = 'boolean' | 'int' | 'float' | 'string';

export interface SettingDescriptor<T = unknown> {
    key: string;
    type: FieldType;
    default: T;
    label: string;
    section: string;
    description?: string;
    min?: number;
    max?: number;
    step?: number;
    options?: {value: string; label: string}[];
}
