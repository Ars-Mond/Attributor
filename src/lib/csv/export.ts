// Typed invoke wrappers for CSV export (feature 009).
import {invoke} from '@tauri-apps/api/core';
import type {CsvPreset, ExportSummary} from './csv';

/** Open a native folder picker; resolves to the chosen directory, or null on cancel. */
export function pickExportDir(): Promise<string | null> {
    return invoke('pick_export_dir');
}

/** Write one CSV per preset into `dir`, using store records for `paths`. */
export function exportCsv(dir: string, paths: string[], presets: CsvPreset[]): Promise<ExportSummary> {
    return invoke('export_csv', {dir, paths, presets});
}
