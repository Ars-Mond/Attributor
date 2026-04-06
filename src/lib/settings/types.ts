import type {Component} from 'svelte';

export type FieldType = 'boolean' | 'int' | 'float' | 'string' | 'custom';

export interface SettingFieldProps {
    descriptor: SettingDescriptor;
    get: () => unknown;
    set: (value: unknown) => void;
    resetToDefault: () => void;
}

export type SettingComponent = Component<SettingFieldProps>;

export interface SettingDescriptor<T = unknown> {
    key: string;    // '' if a custom field stores no value
    type: FieldType;
    default: T;
    label: string;
    description?: string;
    min?: number;
    max?: number;
    step?: number;
    options?: {value: string; label: string}[];
    render?: SettingComponent;   // only for type === 'custom'
}

export interface SettingsSectionConfig {
    id: string;
    label: string;
    icon?: string;
    order?: number;
    component?: unknown;   // typed as SettingSectionComponent in SettingsSection.ts
}
