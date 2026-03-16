<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import FileTree from "$lib/FileTree.svelte";
    import type { FileNode, Metadata } from "$lib/types";

    // --- State ---
    let filename = $state("");
    let title = $state("");
    let description = $state("");
    let keywordInput = $state("");
    let keywords = $state<string[]>([]);
    let categories = $state("");
    let releaseFilename = $state("");
    let autoSave = $state(false);

    // Placeholder image — replaced when a real file is opened
    let imageSrc = $state<string | null>(null);

    // --- File tree ---
    let fileTree = $state<FileNode | null>(null);
    let selectedFilePath = $state("");

    async function openFolder() {
        const result = await invoke<FileNode | null>("open_folder");
        if (result) fileTree = result;
    }

    function selectFile(path: string) {
        selectedFilePath = path;
        // TODO: load image into viewer
    }

    // --- Left panel resize ---
    const LEFT_MIN = 260;
    const LEFT_MAX = 700;
    let panelWidth = $state(380);
    let resizing = $state(false);

    function startResize(e: MouseEvent) {
        e.preventDefault();
        resizing = true;

        function onMove(ev: MouseEvent) {
            panelWidth = Math.min(LEFT_MAX, Math.max(LEFT_MIN, ev.clientX));
        }

        function onUp() {
            resizing = false;
            window.removeEventListener("mousemove", onMove);
            window.removeEventListener("mouseup", onUp);
        }

        window.addEventListener("mousemove", onMove);
        window.addEventListener("mouseup", onUp);
    }

    // --- Right panel resize ---
    const RIGHT_MIN = 160;
    const RIGHT_MAX = 500;
    let rightPanelWidth = $state(240);

    function startRightResize(e: MouseEvent) {
        e.preventDefault();
        resizing = true;

        function onMove(ev: MouseEvent) {
            rightPanelWidth = Math.min(RIGHT_MAX, Math.max(RIGHT_MIN, window.innerWidth - ev.clientX));
        }

        function onUp() {
            resizing = false;
            window.removeEventListener("mousemove", onMove);
            window.removeEventListener("mouseup", onUp);
        }

        window.addEventListener("mousemove", onMove);
        window.addEventListener("mouseup", onUp);
    }

    // --- Preset stock photo keywords grouped by topic ---
    const presets: Record<string, string[]> = {
        Nature: [
            "nature", "landscape", "sky", "water", "forest", "mountain",
            "sunset", "ocean", "river", "flower", "tree", "grass", "cloud",
        ],
        People: [
            "portrait", "woman", "man", "child", "family", "people",
            "lifestyle", "crowd", "face", "smile",
        ],
        Urban: [
            "city", "architecture", "building", "street", "urban",
            "skyline", "road", "bridge",
        ],
        Concepts: [
            "business", "technology", "abstract", "vintage", "minimal",
            "creative", "design", "background", "texture", "pattern",
        ],
        Animals: [
            "dog", "cat", "bird", "wildlife", "animal", "pet", "horse",
        ],
        Seasons: [
            "winter", "summer", "spring", "autumn", "snow", "rain", "fog",
        ],
    };

    // --- Keyword logic ---
    function addKeyword(word: string) {
        const trimmed = word.trim().toLowerCase();
        if (trimmed && !keywords.includes(trimmed)) {
            keywords = [...keywords, trimmed];
        }
    }

    function removeKeyword(word: string) {
        keywords = keywords.filter((k) => k !== word);
    }

    function handleKeywordKeydown(e: KeyboardEvent) {
        if (e.key === "Enter") {
            e.preventDefault();
            addKeyword(keywordInput);
            keywordInput = "";
        }
    }

    function handleKeywordInput() {
        // Add keyword when user types ", " (comma + space)
        if (keywordInput.includes(", ")) {
            const parts = keywordInput.split(", ");
            for (let i = 0; i < parts.length - 1; i++) {
                addKeyword(parts[i]);
            }
            keywordInput = parts[parts.length - 1];
        }
    }

    // --- Save ---
    async function saveMetadata() {
        const metadata: Metadata = {
            filename,
            title,
            description,
            keywords,
            categories,
            releaseFilename,
        };
        await invoke("save_metadata", { metadata });
    }
</script>

<div class="app" class:resizing>
    <!-- ── Left panel ── -->
    <aside class="panel" style="width: {panelWidth}px;">
        <div class="panel-content">
            <h2 class="panel-title">Metadata</h2>

            <!-- Required fields -->
            <section class="field-group">
                <p class="group-label">Required</p>

                <label class="field">
                    <span class="field-label">Filename <span class="required">*</span></span>
                    <input
                        class="input"
                        type="text"
                        placeholder="photo.jpg"
                        bind:value={filename}
                    />
                </label>

                <label class="field">
                    <span class="field-label">Title <span class="required">*</span></span>
                    <input
                        class="input"
                        type="text"
                        placeholder="A stunning mountain sunset"
                        bind:value={title}
                    />
                </label>

                <label class="field">
                    <span class="field-label">Description <span class="required">*</span></span>
                    <textarea
                        class="input textarea"
                        placeholder="Describe the image in detail..."
                        rows={4}
                        bind:value={description}
                    ></textarea>
                </label>

                <!-- Keywords input -->
                <div class="field">
                    <span class="field-label">
                        Keywords <span class="required">*</span>
                        <span class="hint">— press Enter or type ", " to add</span>
                    </span>
                    <input
                        class="input"
                        type="text"
                        placeholder="mountain, sunset, nature..."
                        bind:value={keywordInput}
                        onkeydown={handleKeywordKeydown}
                        oninput={handleKeywordInput}
                    />

                    <!-- Keyword chips -->
                    {#if keywords.length > 0}
                        <div class="keyword-chips">
                            {#each keywords as kw}
                                <span class="chip">
                                    {kw}
                                    <button
                                        class="chip-remove"
                                        onclick={() => removeKeyword(kw)}
                                        aria-label="Remove keyword {kw}"
                                    >×</button>
                                </span>
                            {/each}
                        </div>
                    {/if}
                </div>
            </section>

            <!-- Preset keyword buttons -->
            <section class="field-group presets">
                <p class="group-label">Stock Keywords</p>
                {#each Object.entries(presets) as [group, tags]}
                    <div class="preset-group">
                        <span class="preset-group-label">{group}</span>
                        <div class="preset-tags">
                            {#each tags as tag}
                                <button
                                    class="preset-btn"
                                    class:active={keywords.includes(tag)}
                                    onclick={() => addKeyword(tag)}
                                >{tag}</button>
                            {/each}
                        </div>
                    </div>
                {/each}
            </section>

            <!-- Optional fields (collapsible) -->
            <details class="optional-details">
                <summary class="optional-summary">
                    <span class="group-label" style="border: none; padding: 0;">Optional</span>
                    <svg class="chevron" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M4 6l4 4 4-4"/>
                    </svg>
                </summary>

                <div class="optional-body">
                    <label class="field">
                        <span class="field-label">Categories</span>
                        <input
                            class="input"
                            type="text"
                            placeholder="Travel, Landscape"
                            bind:value={categories}
                        />
                    </label>

                    <label class="field">
                        <span class="field-label">Release Filename</span>
                        <input
                            class="input"
                            type="text"
                            placeholder="model_release.pdf"
                            bind:value={releaseFilename}
                        />
                    </label>
                </div>
            </details>
        </div>

        <!-- Sticky footer: autosave + save button -->
        <footer class="panel-footer">
            <label class="autosave-toggle">
                <input type="checkbox" bind:checked={autoSave} />
                <span>Auto-save</span>
            </label>
            <button class="btn-primary save-btn" onclick={saveMetadata}>Save Changes</button>
        </footer>
    </aside>

    <!-- ── Resize handle ── -->
    <div
        class="resize-handle"
        onmousedown={startResize}
        role="separator"
        aria-label="Resize panel"
        aria-orientation="vertical"
    ></div>

    <!-- ── Center: image viewer ── -->
    <main class="viewer">
        {#if imageSrc}
            <img class="image" src={imageSrc} alt={title || "Preview"} />
        {:else}
            <div class="placeholder">
                <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2">
                    <rect x="3" y="3" width="18" height="18" rx="2" />
                    <circle cx="8.5" cy="8.5" r="1.5" />
                    <path d="M21 15l-5-5L5 21" />
                </svg>
                <p>No image open</p>
            </div>
        {/if}
    </main>

    <!-- ── Right resize handle ── -->
    <div
        class="resize-handle"
        onmousedown={startRightResize}
        role="separator"
        aria-label="Resize file panel"
        aria-orientation="vertical"
    ></div>

    <!-- ── Right panel: file hierarchy ── -->
    <aside class="panel panel--files" style="width: {rightPanelWidth}px;">
        <div class="files-header panel-title">
            <span>Files</span>
            <button class="btn-ghost btn--icon" onclick={openFolder} title="Open folder">
                <svg viewBox="0 0 16 16" fill="currentColor">
                    <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h2.764c.958 0 1.76.56 2.311 1.184C7.985 3.648 8.48 4 9 4h4.5A1.5 1.5 0 0 1 15 5.5v7a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9z"/>
                </svg>
            </button>
        </div>

        <div class="panel-content files-content">
            {#if fileTree}
                <!-- Skip root node, show its children directly -->
                {#each fileTree.children as child (child.path)}
                    <FileTree
                        node={child}
                        depth={0}
                        selectedPath={selectedFilePath}
                        onSelect={selectFile}
                    />
                {/each}
            {:else}
                <div class="files-empty">
                    <svg width="36" height="36" viewBox="0 0 16 16" fill="currentColor">
                        <path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h2.764c.958 0 1.76.56 2.311 1.184C7.985 3.648 8.48 4 9 4h4.5A1.5 1.5 0 0 1 15 5.5v7a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9z"/>
                    </svg>
                    <p>No folder open</p>
                </div>
            {/if}
        </div>
    </aside>
</div>

<style lang="scss">
    @use '../styles/mixins' as *;

    // ── Root layout ──
    .app {
        @include flex(row, flex-start, stretch);
        height: 100vh;
        overflow: hidden;

        &.resizing {
            user-select: none;
            cursor: col-resize;
        }
    }

    // ── Resize handle ──
    .resize-handle {
        width: $handle-width;
        flex-shrink: 0;
        background: $border;
        cursor: col-resize;
        @include transition(background);
        position: relative;

        &::after {
            // Wider invisible hit area for easier grabbing
            content: '';
            position: absolute;
            inset: 0 -4px;
        }

        &:hover { background: $accent; }
    }

    // ── Preset keywords ──
    .presets { padding-bottom: 4px; }

    .preset-group {
        @include flex(column, flex-start, stretch);
        gap: 5px;
    }

    .preset-group-label {
        font-size: $fs-footnote1;
        color: $text-muted;
        font-weight: 500;
    }

    .preset-tags {
        @include flex(row, flex-start, flex-start);
        flex-wrap: wrap;
        gap: 4px;
    }

    .preset-btn {
        @include btn-reset;
        background: $bg-surface;
        border: 1px solid $border;
        border-radius: $radius-sm;
        color: $text-secondary;
        font-size: $fs-footnote1;
        padding: 3px 8px;
        @include transition(background, color, border-color);

        &:hover {
            background: #2e2e2e;
            color: $text;
        }

        &.active {
            background: $chip-bg;
            border-color: $chip-border;
            color: $chip-text;
        }
    }

    // ── Optional spoiler ──
    .optional-details {
        border: 1px solid $border;
        border-radius: $radius-md;
        // No overflow:hidden — it clips the expanded <details> content
    }

    .optional-summary {
        @include flex(row, space-between, center);
        padding: 8px 12px;
        cursor: pointer;
        list-style: none;
        user-select: none;
        background: $bg-surface;
        border-radius: $radius-md;
        @include transition(background);

        &::-webkit-details-marker { display: none; }
        &:hover { background: #2a2a2a; }
    }

    .optional-details[open] .optional-summary {
        border-radius: $radius-md $radius-md 0 0;
    }

    .chevron {
        width: 14px;
        height: 14px;
        color: $text-muted;
        transition: transform 0.2s;
        flex-shrink: 0;
    }

    .optional-details[open] .chevron {
        transform: rotate(180deg);
    }

    .optional-body {
        @include flex(column, flex-start, stretch);
        gap: 12px;
        padding: 12px;
        background: $bg-panel;
        border-radius: 0 0 $radius-md $radius-md;
    }

    // ── Footer controls ──
    .autosave-toggle {
        @include flex(row, flex-start, center);
        gap: 6px;
        cursor: pointer;
        color: $text-secondary;
        font-size: $fs-small;
        white-space: nowrap;

        input[type="checkbox"] {
            accent-color: $accent;
            cursor: pointer;
        }
    }

    // Push save button to the right edge of the footer
    .save-btn { margin-left: auto; }

    // ── Right panel ──
    .panel--files {
        border-left: 1px solid $border;
        border-right: none;
    }

    .files-header {
        @include flex(row, space-between, center);
        padding: 10px 12px;
        border-bottom: 1px solid $border;
        flex-shrink: 0;
        font-size: $fs-small;
        font-weight: 600;
        letter-spacing: 0.04em;
        color: $text-secondary;
        text-transform: uppercase;
    }

    .btn--icon {
        @include flex(row, center, center);
        padding: 4px;
        border-radius: $radius-sm;

        svg {
            width: 14px;
            height: 14px;
            display: block;
        }
    }

    .files-content {
        padding: 6px 4px;
        gap: 1px;
    }

    .files-empty {
        @include flex(column, center, center);
        gap: 10px;
        padding: 40px 16px;
        color: $text-muted;
        opacity: 0.5;
        text-align: center;

        p { font-size: $fs-small; }
    }

    // ── Image viewer ──
    .viewer {
        @include flex(row, center, center, 1);
        overflow: hidden;
        background: $bg-app;
        min-width: 0;
    }

    .image {
        max-width: 100%;
        max-height: 100%;
        object-fit: contain;
        display: block;
    }

    .placeholder {
        @include flex(column, center, center);
        gap: 12px;
        color: $text-muted;

        svg { opacity: 0.4; }
        p   { font-size: $fs-regular; }
    }
</style>
