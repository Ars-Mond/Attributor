import type {Component} from 'svelte';
import type {SettingDescriptor, SettingsSectionConfig} from './types';

export interface SettingSectionProps {
    section: SettingsSection;
    resetSection: () => void;
}

export type SettingSectionComponent = Component<SettingSectionProps>;

export class SettingsSection {
    id: string;
    label: string;
    icon?: string;
    order: number;
    component?: SettingSectionComponent;
    fields: SettingDescriptor[] = [];

    constructor(config: SettingsSectionConfig, fallbackOrder: number) {
        this.id = config.id;
        this.label = config.label;
        this.icon = config.icon;
        this.order = config.order ?? fallbackOrder;
        this.component = config.component as SettingSectionComponent | undefined;
    }
}
