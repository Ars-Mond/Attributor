<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";

    // --- Types ---
    interface Metadata {
        filename: string;
        title: string;
        description: string;
        keywords: string[];
        categories: string;
        releaseFilename: string;
    }

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

    // --- Panel resize ---
    const PANEL_MIN = 260;
    const PANEL_MAX = 700;
    let panelWidth = $state(380);
    let resizing = $state(false);

    function startResize(e: MouseEvent) {
        e.preventDefault();
        resizing = true;

        function onMove(ev: MouseEvent) {
            panelWidth = Math.min(PANEL_MAX, Math.max(PANEL_MIN, ev.clientX));
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
            <button class="save-btn" onclick={saveMetadata}>Save Changes</button>
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

    <!-- ── Right: image viewer ── -->
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
</div>

<style lang="scss">
    // ── Tokens ──
    $bg-app:          #171717;
    $bg-panel:        #1f1f1f;
    $bg-input:        #2a2a2a;
    $bg-input-focus:  #303030;
    $border:          #333;
    $border-focus:    #5a8dee;
    $text:            #e2e2e2;
    $text-secondary:  #888;
    $text-muted:      #555;
    $accent:          #5a8dee;
    $accent-hover:    #7aaaf5;
    $required-color:  #e05c5c;
    $chip-bg:         #1e3a5f;
    $chip-border:     #2e5a8f;
    $preset-active:   #1e3a5f;
    $footer-height:   60px;
    $handle-width:    5px;

    // ── Root layout ──
    :global(*, *::before, *::after) {
        box-sizing: border-box;
        margin: 0;
        padding: 0;
    }

    :global(body) {
        background: $bg-app;
        color: $text;
        font-family: 'Segoe UI', system-ui, sans-serif;
        font-size: 13px;
        height: 100vh;
        overflow: hidden;
    }

    .app {
        display: flex;
        height: 100vh;
        overflow: hidden;

        // Prevent text selection while dragging
        &.resizing {
            user-select: none;
            cursor: col-resize;
        }
    }

    // ── Panel ──
    .panel {
        flex-shrink: 0;
        background: $bg-panel;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        // Width is set via inline style; min/max enforced in JS
    }

    .panel-content {
        flex: 1;
        min-height: 0; // required for flex children to actually scroll
        overflow-y: auto;
        padding: 16px;
        display: flex;
        flex-direction: column;
        gap: 20px;

        &::-webkit-scrollbar       { width: 6px; }
        &::-webkit-scrollbar-track { background: transparent; }
        &::-webkit-scrollbar-thumb { background: #3a3a3a; border-radius: 3px; }
    }

    .panel-title {
        font-size: 15px;
        font-weight: 600;
        color: $text;
        letter-spacing: 0.02em;
    }

    // ── Resize handle ──
    .resize-handle {
        width: $handle-width;
        flex-shrink: 0;
        background: $border;
        cursor: col-resize;
        transition: background 0.15s;
        position: relative;

        &::after {
            // Wider invisible hit area for easier grabbing
            content: '';
            position: absolute;
            inset: 0 -4px;
        }

        &:hover {
            background: $accent;
        }
    }

    // ── Field groups ──
    .field-group {
        display: flex;
        flex-direction: column;
        gap: 12px;
    }

    .group-label {
        font-size: 11px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.08em;
        color: $text-muted;
        border-bottom: 1px solid $border;
        padding-bottom: 6px;
    }

    .field {
        display: flex;
        flex-direction: column;
        gap: 5px;
    }

    .field-label {
        font-size: 12px;
        color: $text-secondary;
        font-weight: 500;
    }

    .required {
        color: $required-color;
        margin-left: 2px;
    }

    .hint {
        color: $text-muted;
        font-weight: 400;
        font-size: 11px;
    }

    // ── Inputs ──
    .input {
        background: $bg-input;
        border: 1px solid $border;
        border-radius: 6px;
        color: $text;
        font-size: 13px;
        padding: 7px 10px;
        outline: none;
        width: 100%;
        transition: border-color 0.15s, background 0.15s;
        font-family: inherit;

        &::placeholder { color: $text-muted; }

        &:focus {
            border-color: $border-focus;
            background: $bg-input-focus;
        }
    }

    .textarea {
        resize: vertical;
        min-height: 80px;
        line-height: 1.5;
    }

    // ── Keyword chips ──
    .keyword-chips {
        display: flex;
        flex-wrap: wrap;
        gap: 5px;
        margin-top: 4px;
    }

    .chip {
        display: inline-flex;
        align-items: center;
        gap: 4px;
        background: $chip-bg;
        border: 1px solid $chip-border;
        border-radius: 4px;
        padding: 3px 8px;
        font-size: 12px;
        color: #8ec5fc;
    }

    .chip-remove {
        background: none;
        border: none;
        color: #8ec5fc;
        cursor: pointer;
        font-size: 14px;
        line-height: 1;
        padding: 0;
        opacity: 0.6;
        transition: opacity 0.1s;

        &:hover { opacity: 1; }
    }

    // ── Preset keywords ──
    .presets { padding-bottom: 4px; }

    .preset-group {
        display: flex;
        flex-direction: column;
        gap: 5px;
    }

    .preset-group-label {
        font-size: 11px;
        color: $text-muted;
        font-weight: 500;
    }

    .preset-tags {
        display: flex;
        flex-wrap: wrap;
        gap: 4px;
    }

    .preset-btn {
        background: #252525;
        border: 1px solid $border;
        border-radius: 4px;
        color: $text-secondary;
        cursor: pointer;
        font-size: 11px;
        padding: 3px 8px;
        transition: background 0.1s, color 0.1s, border-color 0.1s;
        font-family: inherit;

        &:hover {
            background: #2e2e2e;
            color: $text;
        }

        &.active {
            background: $preset-active;
            border-color: $chip-border;
            color: #8ec5fc;
        }
    }

    // ── Optional spoiler ──
    .optional-details {
        border: 1px solid $border;
        border-radius: 6px;
        // No overflow:hidden — it clips the expanded <details> content
    }

    .optional-summary {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 8px 12px;
        cursor: pointer;
        list-style: none;
        user-select: none;
        background: #252525;
        border-radius: 6px; // rounded when closed
        transition: background 0.1s;

        &::-webkit-details-marker { display: none; }

        &:hover { background: #2a2a2a; }
    }

    // When open: only top corners rounded on summary, bottom corners on body
    .optional-details[open] .optional-summary {
        border-radius: 6px 6px 0 0;
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
        display: flex;
        flex-direction: column;
        gap: 12px;
        padding: 12px;
        background: $bg-panel;
        border-radius: 0 0 6px 6px;
    }

    // ── Footer ──
    .panel-footer {
        height: $footer-height;
        min-height: $footer-height;
        border-top: 1px solid $border;
        padding: 0 16px;
        display: flex;
        align-items: center;
        gap: 12px;
        background: $bg-panel;
    }

    .autosave-toggle {
        display: flex;
        align-items: center;
        gap: 6px;
        cursor: pointer;
        color: $text-secondary;
        font-size: 12px;
        white-space: nowrap;

        input[type="checkbox"] {
            accent-color: $accent;
            cursor: pointer;
        }
    }

    .save-btn {
        margin-left: auto;
        background: $accent;
        border: none;
        border-radius: 6px;
        color: #fff;
        cursor: pointer;
        font-size: 13px;
        font-weight: 500;
        padding: 7px 16px;
        transition: background 0.15s;
        font-family: inherit;

        &:hover  { background: $accent-hover; }
        &:active { background: darken($accent, 10%); }
    }

    // ── Image viewer ──
    .viewer {
        flex: 1;
        display: flex;
        align-items: center;
        justify-content: center;
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
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 12px;
        color: $text-muted;

        svg { opacity: 0.4; }
        p   { font-size: 14px; }
    }
</style>
