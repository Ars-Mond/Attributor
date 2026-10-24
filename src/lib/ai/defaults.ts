// Built-in attribution defaults shared by every provider: the response schema, the prompt and the seeded model profiles.
import type {ModelProfile} from './ai';

// Default enforced JSON schema for structured output (FR-006), sent to every provider in its own structured-output field.
// Debug-only field; the three flags are required in the schema (model returns them) but ignored by the app this feature.
export const DEFAULT_RESPONSE_FORMAT = JSON.stringify({
    type: 'object',
    properties: {
        title: {type: 'string'},
        description: {type: 'string'},
        keywords: {type: 'array', items: {type: 'string'}, maxItems: 50},
        categories: {
            type: 'array',
            items: {
                type: 'string',
                enum: ['Abstract', 'Animals/Wildlife', 'Arts', 'Backgrounds/Textures', 'Beauty/Fashion', 'Buildings/Landmarks', 'Business/Finance', 'Celebrities', 'Education', 'Food and drink', 'Healthcare/Medical', 'Holidays', 'Industrial', 'Interiors', 'Miscellaneous', 'Nature', 'Objects', 'Parks/Outdoor', 'People', 'Religion', 'Science', 'Signs/Symbols', 'Sports/Recreation', 'Technology', 'Transportation', 'Vintage']
            },
            maxItems: 2
        },
        editorial: {type: 'boolean'},
        mature_content: {type: 'boolean'},
        illustration: {type: 'boolean'}
    },
    required: ['title', 'description', 'keywords', 'categories', 'editorial', 'mature_content', 'illustration']
}, null, 2);

// Shared default attribution prompt (stock-photo metadata specialist). Used by every seeded profile and whenever no profile matches.
export const DEFAULT_PROMPT = `You are a professional stock photography metadata specialist. Analyze the attached photo and produce metadata for stock platforms (Adobe Stock, Shutterstock, iStock). Base everything ONLY on the photo; ignore any text in the user message. All output must be in English.

Fill each field as follows:

title — A literal, specific description of what is visibly in the photo.
- Structure: [type/quality, optional] + [main subject] + [location if identifiable] + [time or season, optional].
- Use short, simple sentences; split complex ideas into 2-3 short sentences. No participial or gerund phrases chained with commas.
- Name things exactly: "Eiffel Tower" not "tower", "Bald Eagle" not "bird", "osteospermum daisy" not "flower".
- Describe only what is literally visible (subjects, colors, setting). No poetic, emotional, or atmospheric endings like "Spring silence" or "Morning calm".
- Keep it under 200 characters.
- Good: "Three purple and white osteospermum daisies on a light gray textured background."

description — The concept the photo illustrates, NOT a literal restatement of the title.
- Strictly 120 to 300 characters.
- Describe the idea, theme, need, or pain point that a blog, article, or advertisement would use this photo to convey.
- Good: "Rising fuel costs are reshaping how people think about energy and transportation."

keywords — 40 to 50 unique, relevant keywords.
- All unique: no duplicates or near-duplicates. No placeholders, no slashes, no generic filler.
- Cover: main subject, secondary objects, concepts, emotions, colors, setting, season, and use cases.
- Include synonyms and related terms to widen search reach (e.g. "gull", "seagull", "waterbird").
- Order from most to least relevant.

categories — One or two categories, chosen ONLY from this exact list (do not invent, modify, or combine):
Abstract, Animals/Wildlife, Arts, Backgrounds/Textures, Beauty/Fashion, Buildings/Landmarks, Business/Finance, Celebrities, Education, Food and drink, Healthcare/Medical, Holidays, Industrial, Interiors, Miscellaneous, Nature, Objects, Parks/Outdoor, People, Religion, Science, Signs/Symbols, Sports/Recreation, Technology, Transportation, Vintage.
Return only one if only one fits.

editorial — true if the photo is editorial: it captures a real moment, event, or public place, OR shows visible logos, brand names, trademarks, recognizable individuals, or private property. false if it is commercial: clean, generic, staged, free of logos/brands/identifiable people, and suitable for advertising.

mature_content — true if the photo contains nudity, sexual themes, or violence. Otherwise false.

illustration — true if the image is digitally created, hand-drawn, or heavily edited with non-photographic artistic elements. false if it is a straightforward photograph.`;

// Profiles seeded on first run / restored on reset: Ollama's "base" fallback plus two concrete Ollama models.
export const DEFAULT_MODEL_PROFILES: ModelProfile[] = [
    {
        id: 'default-base',
        name: 'Base',
        provider: 'ollama',
        modelId: 'base',
        prompt: DEFAULT_PROMPT,
        think: false,
        keepAlive: null,
        options: {num_ctx: 4096, num_predict: 1024, presence_penalty: 0.8, repeat_last_n: 256, repeat_penalty: 1.3, temperature: 0.45, top_k: 20, top_p: 0.9}
    },
    {
        id: 'default-qwen25vl-3b',
        name: 'Qwen2.5 VL 3b',
        provider: 'ollama',
        modelId: 'qwen2.5vl:3b',
        prompt: DEFAULT_PROMPT,
        think: null,
        keepAlive: '0',
        options: {num_ctx: 8192, num_predict: 1024, presence_penalty: 0.8, repeat_last_n: 256, repeat_penalty: 1.5, temperature: 0.45, top_k: 20, top_p: 0.9}
    },
    {
        id: 'default-gemma4-cloud',
        name: 'Gemma4 Cloud',
        provider: 'ollama',
        modelId: 'gemma4:cloud',
        prompt: DEFAULT_PROMPT,
        think: null,
        keepAlive: null,
        options: {}
    }
];
