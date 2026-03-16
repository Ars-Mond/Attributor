<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import type { Metadata } from "./types";

    // --- State ---
    let filename      = $state("");
    let title         = $state("");
    let description   = $state("");
    let keywordInput  = $state("");
    let keywords      = $state<string[]>([]);
    let categories    = $state("");
    let releaseFilename = $state("");
    let autoSave      = $state(false);

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

<aside class="panel">
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

<style lang="scss">
    @use '../styles/mixins' as *;

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

    .save-btn { margin-left: auto; }
</style>
