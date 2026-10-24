// Localization types and the typed message contract.
// All UI text keys live in `Messages`; en.ts and ru.ts both implement it, so a missing or
// misspelled key is a compile-time error (svelte-check). This is the feature's completeness gate.

export type Locale = 'en' | 'ru';

// The single source of truth for which languages exist. Add a language by appending here and
// providing one catalog file that implements `Messages` (no screen edits required).
export const LOCALES: Locale[] = ['en', 'ru'];

// Fallback for missing keys and for unsupported OS languages on first launch.
export const DEFAULT_LOCALE: Locale = 'en';

// Human-readable language names shown in the selector (endonyms; never translated).
export const ENDONYMS: Record<Locale, string> = {en: 'English', ru: 'Русский'};

// A plural form set. `one` is always required; each language fills the CLDR categories it uses
// (English: one/other; Russian: one/few/many).
export interface PluralForms {
    one: string;
    few?: string;
    many?: string;
    other?: string;
}

// The complete set of user-facing UI strings. Every catalog MUST implement this exactly.
// Plain strings may contain {name} placeholders; PluralForms values are selected by count via tn().
export interface Messages {
    // Shared / generic
    'common.cancel': string;
    'common.save': string;
    'common.close': string;
    'common.copy': string;
    'common.paste': string;
    'common.discard': string;
    'common.dismiss': string;
    'common.required': string;
    'common.reset': string;
    'common.reassign': string;

    // Menu bar
    'menu.file.label': string;
    'menu.file.openDirectory': string;
    'menu.file.settings': string;
    'menu.windows.label': string;
    'menu.windows.showControl': string;
    'menu.windows.showHierarchy': string;
    'menu.help.label': string;
    'menu.help.checkUpdates': string;
    'menu.help.help': string;
    'menu.help.about': string;

    // Theme names (appearance settings) and dock window titles
    'theme.system': string;
    'theme.dark': string;
    'theme.light': string;
    'dock.window.control': string;
    'dock.window.view': string;
    'dock.window.hierarchy': string;

    // Files panel
    'filesPanel.title': string;
    'filesPanel.viewMode.table': string;
    'filesPanel.viewMode.content': string;
    'filesPanel.viewMode.icons': string;
    'filesPanel.layoutDir.vertical': string;
    'filesPanel.layoutDir.horizontal': string;
    'filesPanel.empty.noFolderOpen': string;

    // Image viewer
    'imageViewer.placeholder.noImage': string;
    'imageViewer.loading.ariaLabel': string;
    'imageViewer.gone.message': string;
    'imageViewer.image.alt': string;

    // Dialogs
    'dialog.unsavedChanges.title': string;
    'dialog.unsavedChanges.body': string;
    'dialog.about.versionLabel': string;
    'dialog.about.description': string;
    'dialog.about.identifierLabel': string;
    'dialog.about.licenseLabel': string;
    'dialog.help.loadError': string;
    'dialog.update.title': string;
    'dialog.update.available': string;
    'dialog.update.upToDate': string;
    'dialog.update.error': string;
    'dialog.update.open': string;

    // Metadata / editor panel
    'metadata.title': string;
    'metadata.fileStatus.none': string;
    'metadata.fileStatus.open': string;
    'metadata.fileStatus.edit': string;
    'metadata.fileStatus.app': string;
    'metadata.fileStatus.batch': string;
    'metadata.batch.loading': string;
    'metadata.batch.mixedValues': string;
    'metadata.batch.fileCount': PluralForms;
    'metadata.fieldGroup.required': string;
    'metadata.fieldGroup.fields': string;
    'metadata.stats.words': PluralForms;
    'metadata.stats.chars': PluralForms;
    'metadata.field.filename': string;
    'metadata.field.filename.hint': string;
    'metadata.field.filename.placeholder': string;
    'metadata.field.title': string;
    'metadata.field.title.placeholder': string;
    'metadata.field.description': string;
    'metadata.field.description.placeholder': string;
    'metadata.field.keywords': string;
    'metadata.field.keywords.hint': string;
    'metadata.field.keywords.placeholder': string;
    'metadata.field.keywords.batch.placeholder': string;
    'metadata.button.copy': string;
    'metadata.button.copy.title': string;
    'metadata.button.copy.batch.title': string;
    'metadata.button.paste': string;
    'metadata.button.paste.title': string;
    'metadata.button.clear': string;
    'metadata.button.clear.title': string;
    'metadata.keywords.optionalSection': string;
    'metadata.keywords.stockKeywords.nature': string;
    'metadata.keywords.stockKeywords.people': string;
    'metadata.keywords.stockKeywords.urban': string;
    'metadata.keywords.stockKeywords.concepts': string;
    'metadata.keywords.stockKeywords.animals': string;
    'metadata.keywords.stockKeywords.seasons': string;
    'metadata.optional.section': string;
    'metadata.field.categories': string;
    'metadata.field.categories.placeholder': string;
    'metadata.field.categories.batch.placeholder': string;
    'metadata.field.releaseFilename': string;
    'metadata.field.releaseFilename.placeholder': string;
    'metadata.field.editorial': string;
    'metadata.field.matureContent': string;
    'metadata.field.illustration': string;
    'metadata.validation.noFileSelected': string;
    'metadata.validation.filenameRequired': string;
    'metadata.validation.titleRequired': string;
    'metadata.validation.descriptionRequired': string;
    'metadata.validation.keywordRequired': string;
    'metadata.error.saveFailed': string;
    'metadata.error.copy': string;
    'metadata.error.dismiss': string;
    'metadata.batch.error.failed': string;
    'metadata.batch.error.cancelled': string;
    'metadata.batch.error.of': string;
    'metadata.button.cancel': string;
    'metadata.button.clearAll': string;
    'metadata.dialog.clearKeywords.title': string;
    'metadata.dialog.clearKeywords.body': string;
    'metadata.dialog.clearKeywords.batch.body': PluralForms;
    'metadata.dialog.revert.title': string;
    'metadata.dialog.revert.body': string;
    'metadata.dialog.conflict.title': string;
    'metadata.dialog.conflict.body': string;
    'metadata.dialog.conflict.batch.body': PluralForms;
    'metadata.button.keepApp': string;
    'metadata.button.useFile': string;
    'metadata.button.saveChanges': string;
    'metadata.button.revert': string;
    'metadata.button.revert.title': string;
    'metadata.button.autosave': string;
    'metadata.button.saveBatch': PluralForms;
    'metadata.button.saveBatch.progress': string;
    'metadata.button.cancel.batch': string;
    'metadata.button.cancel.batch.progress': string;
    'metadata.chip.removeKeyword': string;
    'metadata.chip.promoteToAll': string;

    // Shortcuts page + registry action labels
    'shortcuts.action.openFolder': string;
    'shortcuts.action.settings': string;
    'shortcuts.action.save': string;
    'shortcuts.action.copyKeywords': string;
    'shortcuts.action.pasteKeywords': string;
    'shortcuts.action.navUp': string;
    'shortcuts.action.navDown': string;
    'shortcuts.action.navUpExtend': string;
    'shortcuts.action.navDownExtend': string;
    'shortcuts.section.file': string;
    'shortcuts.section.editor': string;
    'shortcuts.section.navigation': string;
    'shortcuts.binding.listening': string;
    'shortcuts.binding.resetToDefault': string;
    'shortcuts.conflict.usedBy': string;
    'shortcuts.resetAll': string;

    // Settings dialog chrome + registry labels/descriptions
    'settings.dialog.title': string;
    'settings.stepper.decrease': string;
    'settings.stepper.increase': string;
    'settings.section.general': string;
    'settings.section.editor': string;
    'settings.section.appearance': string;
    'settings.section.caching': string;
    'settings.section.shortcuts': string;
    'settings.general.language.label': string;
    'settings.general.nestedFolders.label': string;
    'settings.general.nestedFolders.description': string;
    'settings.general.checkUpdates.label': string;
    'settings.general.checkUpdates.description': string;
    'settings.editor.autosave.label': string;
    'settings.editor.autosaveDelay.label': string;
    'settings.appearance.theme.label': string;
    'settings.appearance.fontSize.label': string;
    'settings.caching.photo.label': string;
    'settings.caching.photo.description': string;
    'settings.caching.smallThumbnails.label': string;
    'settings.caching.smallThumbnails.description': string;
    'settings.caching.lazy.label': string;
    'settings.caching.lazy.description': string;

    // AI attribution (features 007, 010)
    'common.create': string;
    'common.edit': string;
    'common.delete': string;
    'settings.section.ai': string;
    'settings.section.modelProfiles': string;
    'settings.ai.provider': string;
    'settings.ai.provider.description.ollama': string;
    'settings.ai.provider.description.local': string;
    'settings.ai.provider.description.cloud': string;
    'settings.ai.status': string;
    'settings.ai.status.reachable': string;
    'settings.ai.status.running': string;
    'settings.ai.status.connected': string;
    'settings.ai.status.notReachable': string;
    'settings.ai.status.installedNotRunning': string;
    'settings.ai.status.notInstalled': string;
    'settings.ai.status.unknown': string;
    'settings.ai.status.failed': string;
    'settings.ai.status.noKey': string;
    'settings.ai.status.signedOut': string;
    'settings.ai.check': string;
    'settings.ai.checking': string;
    'settings.ai.install': string;
    'settings.ai.installing': string;
    'settings.ai.installFailed': string;
    'settings.ai.baseUrl': string;
    'settings.ai.baseUrl.description': string;
    'settings.ai.baseUrl.hint.llamacpp': string;
    'settings.ai.baseUrl.hint.vllm': string;
    'settings.ai.baseUrl.hint.unsloth': string;
    'settings.ai.apiKey': string;
    'settings.ai.apiKey.optional': string;
    'settings.ai.apiKey.saved': string;
    'settings.ai.apiKey.remove': string;
    'settings.ai.apiKey.placeholder': string;
    'settings.ai.apiKey.replace': string;
    'settings.ai.apiKey.get': string;
    'settings.ai.apiKey.hint.llamacpp': string;
    'settings.ai.apiKey.hint.lmstudio': string;
    'settings.ai.apiKey.hint.jan': string;
    'settings.ai.apiKey.hint.vllm': string;
    'settings.ai.apiKey.hint.unsloth': string;
    'settings.ai.apiKey.description': string;
    'settings.ai.apiKey.failed': string;
    'settings.ai.account': string;
    'settings.ai.account.signedIn': string;
    'settings.ai.account.signIn': string;
    'settings.ai.account.otherAccount': string;
    'settings.ai.account.signingIn': string;
    'settings.ai.account.signOut': string;
    'settings.ai.account.manageUsage': string;
    'settings.ai.account.description': string;
    'settings.ai.account.failed': string;
    'settings.ai.activeModel': string;
    'settings.ai.activeModel.none': string;
    'settings.ai.refresh': string;
    'settings.ai.noModels.ollama': string;
    'settings.ai.noModels.other': string;
    'settings.ai.download': string;
    'settings.ai.download.button': string;
    'settings.ai.download.progress': string;
    'settings.ai.responseFormat': string;
    'settings.ai.responseFormat.description': string;
    'settings.ai.answerFormatLabel': string;
    'settings.ai.answerFormatLabel.description': string;
    'settings.modelProfiles.empty': string;
    'settings.modelProfile.createTitle': string;
    'settings.modelProfile.editTitle': string;
    'settings.modelProfile.name': string;
    'settings.modelProfile.provider': string;
    'settings.modelProfile.modelId': string;
    'settings.modelProfile.prompt': string;
    'settings.modelProfile.think': string;
    'settings.modelProfile.keepAlive': string;
    'settings.modelProfile.options': string;
    'settings.modelProfile.options.description.ollama': string;
    'settings.modelProfile.options.description.local': string;
    'settings.modelProfile.options.description.openai': string;
    'settings.modelProfile.options.description.chatgpt': string;
    'settings.modelProfile.options.description.anthropic': string;
    'settings.modelProfile.duplicateModel': string;
    'settings.modelProfile.modelId.hint': string;
    'ai.attribute': string;
    'ai.attribute.tooltip': string;
    'ai.unavailable.tooltip': string;
    'ai.attribute.progress': string;
    'ai.attribute.batch.progress': string;
    'ai.attribute.failed': string;
    'ai.model.installed': string;
    'ai.model.available': string;
    'ai.model.local': string;
    'ai.model.cloud': string;

    // CSV export (feature 009)
    'menu.file.exportCsv': string;
    'dialog.exportCsv.title': string;
    'dialog.exportCsv.resultBody': string;
    'dialog.exportCsv.noPresets': string;
    'dialog.exportCsv.empty': string;
    'settings.section.csv': string;
    'settings.csv.empty': string;
    'settings.csv.createTitle': string;
    'settings.csv.editTitle': string;
    'settings.csv.name': string;
    'settings.csv.identifier': string;
    'settings.csv.identifier.hint': string;
    'settings.csv.delimiter': string;
    'settings.csv.delimiter.comma': string;
    'settings.csv.delimiter.semicolon': string;
    'settings.csv.delimiter.tab': string;
    'settings.csv.fields': string;
    'settings.csv.addField': string;
    'settings.csv.field.column': string;
    'settings.csv.field.column.placeholder': string;
    'settings.csv.field.valueType': string;
    'settings.csv.field.defaultValue': string;
    'settings.csv.field.boolFormat': string;
    'settings.csv.field.moveUp': string;
    'settings.csv.field.moveDown': string;
    'settings.csv.field.remove': string;
    'settings.csv.valueType.none': string;
    'settings.csv.valueType.fileName': string;
    'settings.csv.valueType.title': string;
    'settings.csv.valueType.description': string;
    'settings.csv.valueType.keywords': string;
    'settings.csv.valueType.category': string;
    'settings.csv.valueType.releaseFilename': string;
    'settings.csv.valueType.editorial': string;
    'settings.csv.valueType.matureContent': string;
    'settings.csv.valueType.illustration': string;
    'settings.csv.boolFormat.yesNo': string;
    'settings.csv.boolFormat.trueFalse': string;
    'settings.csv.nameRequired': string;
    'settings.csv.duplicateIdentifier': string;
    'settings.csv.invalidIdentifier': string;
    'settings.csv.fieldsRequired': string;
}

// Keys whose value is a plain string (usable with t()).
export type MessageKey = {[K in keyof Messages]: Messages[K] extends string ? K : never}[keyof Messages];
// Keys whose value is a plural set (usable with tn()).
export type PluralKey = {[K in keyof Messages]: Messages[K] extends PluralForms ? K : never}[keyof Messages];
// Interpolation params for {name}/{n} placeholders.
export type MessageParams = Record<string, string | number>;
