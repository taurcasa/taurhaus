# Infographic regeneration

The architecture diagrams under `docs/images/` are generated from the prompts in
[`docs/images/infographics.manifest.yaml`](../images/infographics.manifest.yaml).
`scripts/generate-infographics.py` regenerates them through the OpenAI Images API.

1. Copy `.env.example` to `.env` (gitignored) and set `OPENAI_API_KEY`. Model, size
   and quality default to `gpt-image-2`, `2048x1152`, `high`; the key is read only by
   this script — never by the app or the daemon.
2. `just infographics-dry-run` — lists the entries marked `stale: true` and the estimated cost.
3. `just infographics` — regenerates them (`--id <name>` for one, `--all` for every entry,
   `--no-reference` to ignore the current image as a style reference). Each result is
   written as a 1600 px-wide JPEG and its manifest entry is updated in place
   (`generation_id`, `sha256`, `updated_at`, `history`; the `stale` keys are removed).
4. Look at every render, then commit the images and the manifest together.

Needs `python3` with PyYAML and Pillow (`sudo apt install python3-yaml python3-pil`).
