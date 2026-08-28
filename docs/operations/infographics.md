# Infographic regeneration

The documentation infographics under `docs/images/` are generated images, and
[`docs/images/infographics.manifest.yaml`](../images/infographics.manifest.yaml)
is their source of truth: one entry per image, carrying the prompt that made it,
the recipe, the checksum, and the generation history. When an image falls behind
the architecture it illustrates, the entry is flagged `stale: true` with a reason
— the prompt is already current, only the pixels are not.

`scripts/generate-infographics.py` (`just infographics`) sends those prompts to
the OpenAI Images API, converts the result to the committed JPEG, and writes the
manifest entry back with targeted text edits so the manifest's comments survive.

## One-time setup

```bash
cp .env.example .env      # repo root; .env is gitignored
$EDITOR .env              # fill in OPENAI_API_KEY
```

`.env` is read by this script and nothing else — neither the taurhaus app nor
the daemon knows the key exists. Real environment variables win over the file,
so `OPENAI_IMAGE_QUALITY=low just infographics` works for a cheap trial run.

| Key | Default | Notes |
|-----|---------|-------|
| `OPENAI_API_KEY` | — | Required. Never printed, never logged. |
| `OPENAI_IMAGE_MODEL` | `gpt-image-2` | Image model. |
| `OPENAI_IMAGE_SIZE` | `1536x1024` | `1024x1024`, `1536x1024`, `1024x1536`, `2048x1152`, `1152x2048`. |
| `OPENAI_IMAGE_QUALITY` | `high` | `low`, `medium`, `high`. Cost scales with this. |
| `OPENAI_BASE_URL` | `https://api.openai.com/v1` | Point at a compatible gateway. |
| `TAURHAUS_INFOGRAPHIC_MAX_WIDTH` | `1600` | Committed JPEGs are downscaled to this width. |
| `TAURHAUS_INFOGRAPHIC_JPEG_QUALITY` | `85` | Progressive, optimized JPEG. |

## Workflow

```bash
just infographics-dry-run                    # what would run, and what it costs
just infographics                            # every `stale: true` entry
just infographics --id scanner-pipeline      # one image (repeatable)
just infographics --all                      # every entry that has a real prompt
git diff --stat docs/images                  # review the bytes...
```

Then look at the JPEGs — the dry run cannot tell you whether the picture is
right, only whether the plan is. When the images are good, commit the images and
the manifest together; they are one change.

Useful flags:

| Flag | Effect |
|------|--------|
| `--dry-run` | Print the selection, model/size/quality, reference use, and a cost estimate. Writes nothing. |
| `--id <image-id>` | Regenerate one entry; repeat for several. Overrides `--stale`. |
| `--all` | Every entry. Entries whose prompt was never reconstructed are skipped and listed. |
| `--no-reference` | Do not attach the current image; generate from the prompt alone. |
| `--keep-png` | Keep the raw API PNG beside the JPEG as `<name>.generated.png` (gitignored). |
| `--price-usd <amount>` | Price the estimate against the current rate card. |

By default an entry with a readable `reference_image_paths` entry goes to
`/images/edits` with the current JPEG attached as a style reference, so the new
image keeps the established dark-teal look. Everything else goes to
`/images/generations`. The prompt sent is the manifest prompt verbatim behind one
line the script owns: *"Regenerate this documentation infographic; keep the
established dark-teal style."*

## What the script writes

- The JPEG at the entry's `output_path`, written atomically (a temp file beside
  the target, then a rename — a killed run never leaves half an image).
- The entry's `generation_id`, `recipe.model`, `recipe.image_size`, `sha256`,
  `updated_at`, and a new `history` line. The `stale` markers are removed once
  the image matches the prompt again.
- One JSON line per image in `docs/images/.generation-log.jsonl` (gitignored):
  id, model, size, quality, checksum, duration, and any usage the API reports.

Everything else in the manifest — the header comments, the prompts, the other
entries — is preserved byte for byte. The manifest is never round-tripped
through a YAML dumper, because a dump would delete every comment in it.

## Editing a prompt

Edit the `prompt:` block in the manifest entry, then regenerate that one image
with `--id`. The prompt in the manifest is what will be sent; there is no second
copy of it anywhere.

## Failure behavior

A failing image does not stop the others: each request gets 180 s and one retry
on 5xx/429, the run finishes the rest of the selection, prints a summary table,
and exits non-zero if anything failed. A failed entry keeps its `stale` markers
and its old image. The API key is redacted from every error path.

## Notes

- `scripts/optimize-doc-image.sh` (ImageMagick) is the older manual path for
  hand-made images. This script does its own conversion with Pillow and does not
  need ImageMagick installed.
- Tests: `just test-scripts` (`python3 -m unittest discover -s scripts/tests`).
  They mock `urllib` — the real API is never called from a test.
