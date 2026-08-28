#!/usr/bin/env python3
"""Regenerate documentation infographics from `docs/images/infographics.manifest.yaml`.

The manifest is the source of truth: every image carries the prompt that made it.
This script reads a prompt, sends it to the OpenAI Images API, converts the PNG
that comes back to the committed JPEG, and writes the result back into the
manifest with targeted text edits so the manifest's comments survive.

Usage:
    just infographics-dry-run          # what would run, and what it would cost
    just infographics                  # regenerate every `stale: true` entry
    just infographics --id data-flow   # one image
    just infographics --all            # every entry that has a real prompt

Configuration comes from `.env` in the repo root (copy `.env.example`); real
environment variables win over the file. The API key is only ever read here —
neither the app nor the daemon knows about it — and is never printed.

Cost estimate
-------------
`--dry-run` prices the run from PRICE_USD below, which mirrors OpenAI's
published per-image image-output pricing for the `gpt-image` family (high
quality: $0.167 square, $0.25 landscape/portrait). Model pricing changes without
this script noticing, so treat the estimate as an order of magnitude and pass
`--price-usd <amount>` to price a run against the current rate card.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import io
import json
import os
import re
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from datetime import date
from pathlib import Path

import yaml
from PIL import Image

REPO_ROOT = Path(__file__).resolve().parents[1]
MANIFEST_RELATIVE = Path("docs/images/infographics.manifest.yaml")
LOG_RELATIVE = Path("docs/images/.generation-log.jsonl")

PROMPT_PREFIX = "Regenerate this documentation infographic; keep the established dark-teal style."

DEFAULT_MODEL = "gpt-image-2"
DEFAULT_SIZE = "1536x1024"
DEFAULT_QUALITY = "high"
DEFAULT_BASE_URL = "https://api.openai.com/v1"
DEFAULT_MAX_WIDTH = 1600
DEFAULT_JPEG_QUALITY = 85

# `gpt-image-1` sizes plus the true 16:9 pair `gpt-image-2` accepts. The list
# only catches typos before a request is paid for; the API is the authority.
ALLOWED_SIZES = ("1024x1024", "1536x1024", "1024x1536", "2048x1152", "1152x2048")
ALLOWED_QUALITIES = ("low", "medium", "high")

REQUEST_TIMEOUT_S = 180
RETRY_BACKOFF_S = 5
RETRYABLE_STATUS = (408, 409, 429)

# Assumed per-image price in USD, keyed by (quality, size). See "Cost estimate".
PRICE_USD = {
    ("low", "1024x1024"): 0.011,
    ("low", "1536x1024"): 0.016,
    ("low", "1024x1536"): 0.016,
    ("medium", "1024x1024"): 0.042,
    ("medium", "1536x1024"): 0.063,
    ("medium", "1024x1536"): 0.063,
    ("high", "1024x1024"): 0.167,
    ("high", "1536x1024"): 0.250,
    ("high", "1024x1536"): 0.250,
    # The 16:9 pair is priced here at the landscape tier — an assumption, not a
    # published rate. Pass --price-usd to price a run properly.
    ("low", "2048x1152"): 0.016,
    ("low", "1152x2048"): 0.016,
    ("medium", "2048x1152"): 0.063,
    ("medium", "1152x2048"): 0.063,
    ("high", "2048x1152"): 0.250,
    ("high", "1152x2048"): 0.250,
}

# Entries whose prompt was never reconstructed carry this marker instead of one.
PLACEHOLDER_PROMPT_MARKER = "prompt not available"

REDACTED = "***redacted***"


class ConfigError(Exception):
    """The `.env`/environment configuration cannot produce a usable run."""


class SelectionError(Exception):
    """The requested image ids do not exist in the manifest."""


class ManifestEditError(Exception):
    """The manifest entry does not have the shape the targeted edits need."""


# --------------------------------------------------------------------------- #
# Configuration
# --------------------------------------------------------------------------- #


@dataclass(frozen=True)
class Config:
    api_key: str
    model: str
    size: str
    quality: str
    base_url: str
    max_width: int
    jpeg_quality: int


def parse_env_file(text):
    """Parse a `.env` body: `KEY=VALUE`, `#` comments, optional quotes."""
    values = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("export "):
            line = line[len("export ") :].lstrip()
        key, separator, value = line.partition("=")
        if not separator:
            continue
        key = key.strip()
        if not key:
            continue
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
            value = value[1:-1]
        else:
            # An unquoted value ends at a whitespace-preceded `#`.
            value = re.split(r"\s+#", value, maxsplit=1)[0].strip()
        values[key] = value
    return values


def load_env_file(root):
    path = Path(root) / ".env"
    if not path.is_file():
        return {}
    return parse_env_file(path.read_text(encoding="utf-8"))


def _pick(file_values, environ, key, fallback):
    value = environ.get(key) or file_values.get(key) or ""
    value = value.strip()
    return value or fallback


def _positive_int(raw, key, fallback):
    if raw in (None, ""):
        return fallback
    try:
        value = int(raw)
    except (TypeError, ValueError) as error:
        raise ConfigError(f"{key} must be a positive integer, got {raw!r}") from error
    if value <= 0:
        raise ConfigError(f"{key} must be a positive integer, got {raw!r}")
    return value


def resolve_config(file_values, environ, require_key=True):
    """Merge `.env` values with the environment (environment wins) and validate."""
    api_key = _pick(file_values, environ, "OPENAI_API_KEY", "")
    if require_key and not api_key:
        raise ConfigError(
            "OPENAI_API_KEY is not set. Copy .env.example to .env in the repo root "
            "and fill in the key (see docs/operations/infographics.md)."
        )

    size = _pick(file_values, environ, "OPENAI_IMAGE_SIZE", DEFAULT_SIZE)
    if size not in ALLOWED_SIZES:
        raise ConfigError(f"OPENAI_IMAGE_SIZE must be one of {' | '.join(ALLOWED_SIZES)}, got {size!r}")

    quality = _pick(file_values, environ, "OPENAI_IMAGE_QUALITY", DEFAULT_QUALITY)
    if quality not in ALLOWED_QUALITIES:
        raise ConfigError(
            f"OPENAI_IMAGE_QUALITY must be one of {' | '.join(ALLOWED_QUALITIES)}, got {quality!r}"
        )

    return Config(
        api_key=api_key,
        model=_pick(file_values, environ, "OPENAI_IMAGE_MODEL", DEFAULT_MODEL),
        size=size,
        quality=quality,
        base_url=_pick(file_values, environ, "OPENAI_BASE_URL", DEFAULT_BASE_URL).rstrip("/"),
        max_width=_positive_int(
            _pick(file_values, environ, "TAURHAUS_INFOGRAPHIC_MAX_WIDTH", ""),
            "TAURHAUS_INFOGRAPHIC_MAX_WIDTH",
            DEFAULT_MAX_WIDTH,
        ),
        jpeg_quality=_positive_int(
            _pick(file_values, environ, "TAURHAUS_INFOGRAPHIC_JPEG_QUALITY", ""),
            "TAURHAUS_INFOGRAPHIC_JPEG_QUALITY",
            DEFAULT_JPEG_QUALITY,
        ),
    )


# --------------------------------------------------------------------------- #
# Manifest reading and selection
# --------------------------------------------------------------------------- #


def manifest_path(root):
    return Path(root) / MANIFEST_RELATIVE


def parse_manifest_text(text):
    return yaml.safe_load(text)


def load_manifest(root):
    return parse_manifest_text(manifest_path(root).read_text(encoding="utf-8"))


def has_usable_prompt(entry):
    prompt = (entry.get("recipe") or {}).get("prompt") or ""
    return bool(prompt.strip()) and PLACEHOLDER_PROMPT_MARKER not in prompt


def select_entries(manifest, mode, ids):
    images = manifest.get("images") or {}
    if mode == "id":
        missing = [image_id for image_id in ids if image_id not in images]
        if missing:
            raise SelectionError(f"Unknown image id(s): {', '.join(missing)}")
        return [(image_id, images[image_id]) for image_id in ids]
    if mode == "all":
        return list(images.items())
    return [(image_id, entry) for image_id, entry in images.items() if entry.get("stale")]


def reference_image_for(root, entry):
    for relative in (entry.get("recipe") or {}).get("reference_image_paths") or []:
        path = Path(root) / relative
        if path.is_file() and os.access(path, os.R_OK):
            return path
    return None


def price_for(config, override):
    if override is not None:
        return override
    return PRICE_USD.get((config.quality, config.size), PRICE_USD[("high", "1536x1024")])


# --------------------------------------------------------------------------- #
# Requests
# --------------------------------------------------------------------------- #


def build_prompt(manifest_prompt):
    return f"{PROMPT_PREFIX}\n\n{manifest_prompt}"


def _auth_headers(config):
    return {"Authorization": f"Bearer {config.api_key}"}


def build_generation_request(config, prompt):
    body = json.dumps(
        {
            "model": config.model,
            "prompt": prompt,
            "size": config.size,
            "quality": config.quality,
            "n": 1,
        }
    ).encode("utf-8")
    headers = _auth_headers(config)
    headers["Content-Type"] = "application/json"
    return urllib.request.Request(
        f"{config.base_url}/images/generations", data=body, headers=headers, method="POST"
    )


def build_edit_request(config, prompt, reference_path):
    reference_path = Path(reference_path)
    boundary = f"----taurhaus{binascii.hexlify(os.urandom(16)).decode('ascii')}"
    fields = [
        ("model", config.model),
        ("prompt", prompt),
        ("size", config.size),
        ("quality", config.quality),
        ("n", "1"),
    ]
    chunks = []
    for name, value in fields:
        chunks.append(f"--{boundary}\r\n".encode("utf-8"))
        chunks.append(f'Content-Disposition: form-data; name="{name}"\r\n\r\n'.encode("utf-8"))
        chunks.append(f"{value}\r\n".encode("utf-8"))
    chunks.append(f"--{boundary}\r\n".encode("utf-8"))
    chunks.append(
        f'Content-Disposition: form-data; name="image[]"; filename="{reference_path.name}"\r\n'.encode(
            "utf-8"
        )
    )
    chunks.append(b"Content-Type: image/jpeg\r\n\r\n")
    chunks.append(reference_path.read_bytes())
    chunks.append(b"\r\n")
    chunks.append(f"--{boundary}--\r\n".encode("utf-8"))

    headers = _auth_headers(config)
    headers["Content-Type"] = f"multipart/form-data; boundary={boundary}"
    return urllib.request.Request(
        f"{config.base_url}/images/edits", data=b"".join(chunks), headers=headers, method="POST"
    )


def describe_request(request):
    """One-line request description with the bearer token redacted."""
    headers = []
    for name, value in sorted(request.headers.items()):
        if name.lower() == "authorization":
            value = f"Bearer {REDACTED}"
        headers.append(f"{name}: {value}")
    return f"{request.get_method()} {request.full_url} [{'; '.join(headers)}]"


def redact(text, secret):
    if secret and secret in text:
        return text.replace(secret, REDACTED)
    return text


def post_with_retry(request, secret="", timeout=REQUEST_TIMEOUT_S):
    """POST once, retry once on 5xx/429, and raise with the key redacted."""
    attempts = 0
    while True:
        attempts += 1
        try:
            with urllib.request.urlopen(request, timeout=timeout) as response:
                return json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as error:
            retryable = error.code >= 500 or error.code in RETRYABLE_STATUS
            if retryable and attempts == 1:
                time.sleep(RETRY_BACKOFF_S)
                continue
            detail = ""
            try:
                detail = error.read().decode("utf-8", "replace")[:400]
            except Exception:  # noqa: BLE001 - a body is best effort in an error path
                detail = ""
            raise RuntimeError(
                redact(f"HTTP {error.code} from {describe_request(request)}: {detail}", secret)
            ) from None
        except urllib.error.URLError as error:
            if attempts == 1:
                time.sleep(RETRY_BACKOFF_S)
                continue
            raise RuntimeError(
                redact(f"Request failed for {describe_request(request)}: {error.reason}", secret)
            ) from None


def decode_image_payload(payload):
    data = (payload or {}).get("data") or []
    if not data or not data[0].get("b64_json"):
        raise RuntimeError("The API response carried no image data (data[0].b64_json missing).")
    return base64.b64decode(data[0]["b64_json"])


# --------------------------------------------------------------------------- #
# Image conversion and writing
# --------------------------------------------------------------------------- #


def png_to_jpeg(png_data, max_width, quality):
    with Image.open(io.BytesIO(png_data)) as image:
        image = image.convert("RGB")
        if image.width > max_width:
            height = max(1, round(image.height * max_width / image.width))
            image = image.resize((max_width, height), Image.LANCZOS)
        buffer = io.BytesIO()
        image.save(buffer, format="JPEG", quality=quality, progressive=True, optimize=True)
    return buffer.getvalue()


def write_atomic(path, data):
    """Write `data` to `path` via a sibling temp file so readers never see a partial file."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    handle = tempfile.NamedTemporaryFile(
        dir=path.parent, prefix=f".{path.name}.", suffix=".tmp", delete=False
    )
    temp_path = Path(handle.name)
    try:
        with handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temp_path, path)
    except BaseException:
        temp_path.unlink(missing_ok=True)
        raise


def sha256_hex(data):
    return hashlib.sha256(data).hexdigest()


# --------------------------------------------------------------------------- #
# Targeted manifest edits (never a YAML dump — the comments are the docs)
# --------------------------------------------------------------------------- #


def _entry_span(lines, image_id):
    start = None
    pattern = re.compile(rf"^  {re.escape(image_id)}:\s*$")
    for index, line in enumerate(lines):
        if pattern.match(line):
            start = index
            break
    if start is None:
        raise ManifestEditError(f"No manifest entry for {image_id!r}")
    for index in range(start + 1, len(lines)):
        if re.match(r"^  \S", lines[index]):
            return start, index
    return start, len(lines)


def _prompt_span(block):
    """Index range of the `prompt:` block scalar, so field edits skip its body."""
    for index, line in enumerate(block):
        if re.match(r"^      prompt:", line):
            end = index + 1
            while end < len(block) and (not block[end].strip() or re.match(r"^ {8,}", block[end])):
                end += 1
            return index, end
    return -1, -1


def _replace_field(block, pattern, replacement, image_id):
    prompt_start, prompt_end = _prompt_span(block)
    for index, line in enumerate(block):
        if prompt_start <= index < prompt_end:
            continue
        if re.match(pattern, line):
            block[index] = replacement
            return
    raise ManifestEditError(f"{image_id}: no line matching {pattern!r} in the entry")


def _drop_stale_markers(block):
    result = []
    index = 0
    while index < len(block):
        line = block[index]
        stripped = line.strip()
        if re.match(r"^    stale(_since)?:", line):
            index += 1
            continue
        if re.match(r"^    stale_reason:", line):
            index += 1
            while index < len(block) and re.match(r"^ {6,}\S", block[index]):
                index += 1
            continue
        if stripped.startswith("#") and "sha256 describes the STALE" in stripped:
            index += 1
            continue
        if stripped.startswith("# STALE PIXELS"):
            # The marker is a comment block; it goes with the keys it explains.
            index += 1
            while index < len(block) and block[index].strip().startswith("#"):
                index += 1
            continue
        result.append(line)
        index += 1
    return result


def _append_history(block, image_id, generation_id, comment):
    entry_line = f"      - {generation_id}  # {comment}\n"
    for index, line in enumerate(block):
        if not re.match(r"^    history:", line):
            continue
        if line.strip() == "history: []":
            block[index : index + 1] = ["    history:\n", entry_line]
            return
        insert_at = index + 1
        while insert_at < len(block) and re.match(r"^      - ", block[insert_at]):
            insert_at += 1
        block.insert(insert_at, entry_line)
        return
    raise ManifestEditError(f"{image_id}: no history list in the entry")


def update_manifest_text(
    text,
    image_id,
    *,
    generation_id,
    model,
    image_size,
    sha256,
    updated_at,
    history_comment,
):
    """Rewrite one entry in place; every other byte of the manifest is preserved."""
    lines = text.splitlines(keepends=True)
    start, end = _entry_span(lines, image_id)
    block = lines[start:end]

    block = _drop_stale_markers(block)
    _replace_field(block, r"^    generation_id:", f"    generation_id: {generation_id}\n", image_id)
    _replace_field(block, r"^      model:", f"      model: {model}\n", image_id)
    _replace_field(block, r"^      image_size:", f'      image_size: "{image_size}"\n', image_id)
    _replace_field(block, r"^    sha256:", f"    sha256: {sha256}\n", image_id)
    _replace_field(block, r"^    updated_at:", f'    updated_at: "{updated_at}"\n', image_id)
    _append_history(block, image_id, generation_id, history_comment)

    return "".join(lines[:start] + block + lines[end:])


# --------------------------------------------------------------------------- #
# CLI
# --------------------------------------------------------------------------- #


def parse_args(argv):
    parser = argparse.ArgumentParser(
        prog="generate-infographics.py",
        description="Regenerate documentation infographics from the manifest prompts.",
    )
    scope = parser.add_mutually_exclusive_group()
    scope.add_argument("--stale", action="store_true", help="every entry flagged stale (default)")
    scope.add_argument("--all", action="store_true", help="every entry that has a real prompt")
    parser.add_argument(
        "--id", dest="ids", action="append", default=[], metavar="IMAGE_ID", help="one image id (repeatable)"
    )
    parser.add_argument("--dry-run", action="store_true", help="print the plan and the cost estimate")
    parser.add_argument("--no-reference", action="store_true", help="never attach the current image")
    parser.add_argument("--keep-png", action="store_true", help="keep the raw PNG beside the JPEG")
    parser.add_argument(
        "--price-usd", type=float, default=None, metavar="USD", help="override the assumed per-image price"
    )
    return parser.parse_args(argv)


def _fail(message):
    print(f"error: {message}", file=sys.stderr)
    return 2


def _print_plan(selected, skipped, config, price, use_reference, root):
    print(f"Model {config.model} · size {config.size} · quality {config.quality}")
    print(f"Output: max width {config.max_width}px · JPEG quality {config.jpeg_quality}")
    print(f"{len(selected)} image(s) selected:")
    for image_id, entry in selected:
        reference = reference_image_for(root, entry) if use_reference else None
        route = f"edits + reference {reference.name}" if reference else "generations (no reference)"
        print(f"  {image_id:<32} {entry.get('output_path', '?'):<48} {route}")
    for image_id, reason in skipped:
        print(f"  {image_id:<32} skipped — {reason}")
    print(f"Estimated cost: {len(selected)} x ${price:.2f} = ${len(selected) * price:.2f}")


def _log_record(root, record):
    path = Path(root) / LOG_RELATIVE
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(record, sort_keys=True) + "\n")


def _generate_one(root, config, image_id, entry, args):
    """Generate, convert, write, and return the log record for one image."""
    prompt = build_prompt((entry.get("recipe") or {}).get("prompt") or "")
    reference = None if args.no_reference else reference_image_for(root, entry)
    request = (
        build_edit_request(config, prompt, reference)
        if reference
        else build_generation_request(config, prompt)
    )

    started = time.monotonic()
    payload = post_with_retry(request, secret=config.api_key)
    png_data = decode_image_payload(payload)
    duration = round(time.monotonic() - started, 2)

    output_path = Path(root) / entry["output_path"]
    if args.keep_png:
        write_atomic(output_path.with_suffix("").with_name(f"{output_path.stem}.generated.png"), png_data)

    jpeg = png_to_jpeg(png_data, config.max_width, config.jpeg_quality)
    write_atomic(output_path, jpeg)
    digest = sha256_hex(jpeg)

    with Image.open(io.BytesIO(jpeg)) as image:
        dimensions = f"{image.width}x{image.height}"

    today = date.today().isoformat()
    path = manifest_path(root)
    write_atomic(
        path,
        update_manifest_text(
            path.read_text(encoding="utf-8"),
            image_id,
            generation_id=f"gen_{digest[:12]}",
            model=config.model,
            image_size=config.size,
            sha256=digest,
            updated_at=today,
            history_comment=f"regenerated {today} via openai {config.model}",
        ).encode("utf-8"),
    )

    record = {
        "id": image_id,
        "model": config.model,
        "size": config.size,
        "quality": config.quality,
        "sha256": digest,
        "bytes": len(jpeg),
        "dimensions": dimensions,
        "duration_s": duration,
        "reference": str(reference.relative_to(root)) if reference else None,
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
    }
    usage = payload.get("usage")
    if usage:
        record["usage"] = usage
    return record


def main(argv=None, repo_root=None):
    args = parse_args(argv)
    root = Path(repo_root) if repo_root else REPO_ROOT

    try:
        config = resolve_config(load_env_file(root), os.environ, require_key=not args.dry_run)
    except ConfigError as error:
        return _fail(str(error))

    try:
        manifest = load_manifest(root)
    except OSError as error:
        return _fail(f"cannot read {manifest_path(root)}: {error}")

    mode = "id" if args.ids else ("all" if args.all else "stale")
    try:
        candidates = select_entries(manifest, mode, args.ids)
    except SelectionError as error:
        return _fail(str(error))

    selected, skipped = [], []
    for image_id, entry in candidates:
        if has_usable_prompt(entry):
            selected.append((image_id, entry))
        else:
            skipped.append((image_id, "the manifest has no prompt for this image"))

    if not selected:
        print("Nothing to regenerate.")
        for image_id, reason in skipped:
            print(f"  {image_id} skipped — {reason}")
        return 1 if skipped and mode == "id" else 0

    if args.dry_run:
        _print_plan(selected, skipped, config, price_for(config, args.price_usd), not args.no_reference, root)
        print("Dry run — nothing was written.")
        return 0

    results = []
    for image_id, entry in selected:
        try:
            record = _generate_one(root, config, image_id, entry, args)
        except (RuntimeError, OSError, ValueError) as error:
            message = redact(str(error), config.api_key)
            print(f"error: {image_id}: {message}", file=sys.stderr)
            results.append((image_id, "FAILED", message))
            continue
        _log_record(root, record)
        results.append(
            (image_id, "ok", f"{record['dimensions']} · {record['bytes'] / 1024:.0f} KB · {record['duration_s']}s")
        )

    print("\nSummary")
    for image_id, status, detail in results:
        print(f"  {status:<7} {image_id:<32} {detail}")
    for image_id, reason in skipped:
        print(f"  skipped {image_id:<32} {reason}")

    failures = [item for item in results if item[1] != "ok"]
    if failures:
        print(f"{len(failures)} image(s) failed.", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
