"""Tests for scripts/generate-infographics.py.

Run with: python3 -m unittest discover -s scripts/tests

The real OpenAI API is never called here: every test that exercises a request
patches `urllib.request.urlopen` inside the script module.
"""

import base64
import importlib.util
import io
import json
import os
import re
import sys
import tempfile
import time
import unittest
import urllib.error
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

from PIL import Image

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "generate-infographics.py"
REAL_MANIFEST = REPO_ROOT / "docs" / "images" / "infographics.manifest.yaml"


def _load_script_module():
    spec = importlib.util.spec_from_file_location("generate_infographics", SCRIPT_PATH)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


gen = _load_script_module()

FAKE_KEY = "sk-test-not-a-real-key-0000"


def png_bytes(width=2400, height=1600):
    image = Image.new("RGB", (width, height), (12, 46, 52))
    for x in range(0, width, 240):
        for y in range(0, height, 160):
            image.putpixel((x, y), (120, 220, 210))
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    return buffer.getvalue()


def make_repo(tmpdir, manifest_text=None, env_text=None):
    """Build a throwaway repo root with a manifest copy and reference images."""
    root = Path(tmpdir)
    images = root / "docs" / "images"
    images.mkdir(parents=True)
    text = manifest_text if manifest_text is not None else REAL_MANIFEST.read_text(encoding="utf-8")
    (images / "infographics.manifest.yaml").write_text(text, encoding="utf-8")
    if env_text is not None:
        (root / ".env").write_text(env_text, encoding="utf-8")
    manifest = gen.load_manifest(root)
    for entry in manifest["images"].values():
        for ref in entry.get("recipe", {}).get("reference_image_paths") or []:
            path = root / ref
            path.parent.mkdir(parents=True, exist_ok=True)
            Image.new("RGB", (64, 40), (12, 46, 52)).save(path, format="JPEG")
    return root


def entry_blocks(text):
    """Split a manifest into {image_id: block text} plus a '__header__' slice."""
    lines = text.splitlines(keepends=True)
    blocks = {"__header__": []}
    current = "__header__"
    for line in lines:
        match = re.match(r"^  ([a-z0-9][a-z0-9-]*):\s*$", line)
        if match:
            current = match.group(1)
            blocks[current] = []
        blocks[current].append(line)
    return {key: "".join(value) for key, value in blocks.items()}


class FakeResponse(io.BytesIO):
    """A response that reads once, like a socket — not a mock that repeats itself."""

    def __enter__(self):
        return self

    def __exit__(self, *args):
        return False


def fake_response(png=None, payload=None):
    body = (
        payload
        if payload is not None
        else {"data": [{"b64_json": base64.b64encode(png or png_bytes()).decode("ascii")}]}
    )
    return FakeResponse(json.dumps(body).encode("utf-8"))


def fake_urlopen(png=None, payload=None):
    """A urlopen stand-in that hands every call its own unread response."""

    def urlopen(request, timeout=None):
        return fake_response(png, payload)

    return urlopen


class EnvParsingTests(unittest.TestCase):
    def test_parses_comments_quotes_and_blank_lines(self):
        text = "\n".join(
            [
                "# leading comment",
                "",
                "OPENAI_API_KEY=plain-value",
                'OPENAI_IMAGE_MODEL="gpt-image-2"',
                "OPENAI_IMAGE_SIZE='1024x1024'",
                "OPENAI_BASE_URL=https://example.test/v1  # trailing comment",
                "  OPENAI_IMAGE_QUALITY = medium ",
                "NOT_A_PAIR",
            ]
        )
        values = gen.parse_env_file(text)
        self.assertEqual(values["OPENAI_API_KEY"], "plain-value")
        self.assertEqual(values["OPENAI_IMAGE_MODEL"], "gpt-image-2")
        self.assertEqual(values["OPENAI_IMAGE_SIZE"], "1024x1024")
        self.assertEqual(values["OPENAI_BASE_URL"], "https://example.test/v1")
        self.assertEqual(values["OPENAI_IMAGE_QUALITY"], "medium")
        self.assertNotIn("NOT_A_PAIR", values)

    def test_keeps_a_hash_inside_a_quoted_value(self):
        values = gen.parse_env_file('OPENAI_API_KEY="secret#1"')
        self.assertEqual(values["OPENAI_API_KEY"], "secret#1")

    def test_environment_overrides_the_env_file(self):
        file_values = {"OPENAI_API_KEY": "from-file", "OPENAI_IMAGE_QUALITY": "low"}
        config = gen.resolve_config(file_values, {"OPENAI_API_KEY": "from-environ"})
        self.assertEqual(config.api_key, "from-environ")
        self.assertEqual(config.quality, "low")

    def test_missing_key_raises_a_clear_error_without_leaking_values(self):
        with self.assertRaises(gen.ConfigError) as caught:
            gen.resolve_config({"OPENAI_IMAGE_MODEL": "gpt-image-2"}, {})
        message = str(caught.exception)
        self.assertIn("OPENAI_API_KEY", message)
        self.assertIn(".env", message)

    def test_accepts_the_16_9_size_gpt_image_2_takes(self):
        config = gen.resolve_config({"OPENAI_API_KEY": FAKE_KEY, "OPENAI_IMAGE_SIZE": "2048x1152"}, {})
        self.assertEqual(config.size, "2048x1152")
        self.assertGreater(gen.price_for(config, None), 0)

    def test_the_default_size_is_the_shape_the_manifest_declares(self):
        # Regression: 8541bf2 shipped DEFAULT_SIZE = "1536x1024" (3:2) while every
        # manifest entry declares aspect_ratio "16:9", so the documented default run
        # would have reshaped all eight stale infographics.
        config = gen.resolve_config({"OPENAI_API_KEY": FAKE_KEY}, {})
        self.assertEqual(config.size, "2048x1152")
        self.assertEqual(gen.aspect_ratio_for(config.size), "16:9")

    def test_every_stale_entry_resolves_to_its_declared_ratio_by_default(self):
        # Regression: 8541bf2 — the default geometry has to satisfy the manifest.
        config = gen.resolve_config({"OPENAI_API_KEY": FAKE_KEY}, {})
        stale = gen.select_entries(gen.load_manifest(REPO_ROOT), "stale", [])
        self.assertTrue(stale)
        for image_id, entry in stale:
            self.assertEqual(entry["recipe"]["aspect_ratio"], gen.aspect_ratio_for(config.size), image_id)

    def test_a_ratio_is_reduced_from_the_pixel_size(self):
        self.assertEqual(gen.aspect_ratio_for("2048x1152"), "16:9")
        self.assertEqual(gen.aspect_ratio_for("1536x1024"), "3:2")
        self.assertEqual(gen.aspect_ratio_for("1024x1024"), "1:1")
        self.assertEqual(gen.aspect_ratio_for("1152x2048"), "9:16")

    def test_rejects_an_unsupported_size_and_quality(self):
        with self.assertRaises(gen.ConfigError):
            gen.resolve_config({"OPENAI_API_KEY": FAKE_KEY, "OPENAI_IMAGE_SIZE": "4096x4096"}, {})
        with self.assertRaises(gen.ConfigError):
            gen.resolve_config({"OPENAI_API_KEY": FAKE_KEY, "OPENAI_IMAGE_QUALITY": "ultra"}, {})

    def test_defaults_match_the_committed_example_file(self):
        config = gen.resolve_config({"OPENAI_API_KEY": FAKE_KEY}, {})
        example = gen.parse_env_file((REPO_ROOT / ".env.example").read_text(encoding="utf-8"))
        self.assertEqual(config.model, example["OPENAI_IMAGE_MODEL"])
        self.assertEqual(config.size, example["OPENAI_IMAGE_SIZE"])
        self.assertEqual(config.quality, example["OPENAI_IMAGE_QUALITY"])
        self.assertEqual(str(config.max_width), example["TAURHAUS_INFOGRAPHIC_MAX_WIDTH"])
        self.assertEqual(str(config.jpeg_quality), example["TAURHAUS_INFOGRAPHIC_JPEG_QUALITY"])


class SelectionTests(unittest.TestCase):
    def setUp(self):
        self.manifest = gen.load_manifest(REPO_ROOT)

    def test_stale_selection_takes_only_flagged_entries(self):
        selected = [image_id for image_id, _ in gen.select_entries(self.manifest, "stale", [])]
        self.assertTrue(selected)
        for image_id in selected:
            self.assertTrue(self.manifest["images"][image_id].get("stale"))
        flagged = [key for key, entry in self.manifest["images"].items() if entry.get("stale")]
        self.assertEqual(sorted(selected), sorted(flagged))

    def test_all_selection_takes_every_entry(self):
        selected = [image_id for image_id, _ in gen.select_entries(self.manifest, "all", [])]
        self.assertEqual(sorted(selected), sorted(self.manifest["images"]))

    def test_entries_without_a_reconstructed_prompt_are_not_usable(self):
        images = self.manifest["images"]
        self.assertFalse(gen.has_usable_prompt(images["command-center-flow"]))
        self.assertTrue(gen.has_usable_prompt(images["data-model"]))

    def test_id_selection_keeps_the_requested_order(self):
        selected = [
            image_id
            for image_id, _ in gen.select_entries(self.manifest, "id", ["scanner-pipeline", "data-model"])
        ]
        self.assertEqual(selected, ["scanner-pipeline", "data-model"])

    def test_unknown_id_is_an_error(self):
        with self.assertRaises(gen.SelectionError):
            gen.select_entries(self.manifest, "id", ["no-such-image"])


class RequestBuildingTests(unittest.TestCase):
    def setUp(self):
        self.config = gen.resolve_config({"OPENAI_API_KEY": FAKE_KEY}, {})

    def test_generation_request_posts_json(self):
        request = gen.build_generation_request(self.config, "PROMPT TEXT")
        self.assertEqual(request.full_url, f"{self.config.base_url}/images/generations")
        self.assertEqual(request.get_header("Content-type"), "application/json")
        body = json.loads(request.data.decode("utf-8"))
        self.assertEqual(
            body,
            {
                "model": self.config.model,
                "prompt": "PROMPT TEXT",
                "size": self.config.size,
                "quality": self.config.quality,
                "n": 1,
            },
        )

    def test_edit_request_posts_multipart_with_the_reference_image(self):
        with tempfile.TemporaryDirectory() as tmp:
            reference = Path(tmp) / "reference.jpg"
            Image.new("RGB", (32, 20), (12, 46, 52)).save(reference, format="JPEG")
            request = gen.build_edit_request(self.config, "PROMPT TEXT", reference)
        self.assertEqual(request.full_url, f"{self.config.base_url}/images/edits")
        self.assertIn("multipart/form-data; boundary=", request.get_header("Content-type"))
        body = request.data.decode("latin-1")
        self.assertIn('name="model"', body)
        self.assertIn(self.config.model, body)
        self.assertIn('name="prompt"', body)
        self.assertIn("PROMPT TEXT", body)
        self.assertIn('name="size"', body)
        self.assertIn('name="quality"', body)
        self.assertIn('name="n"', body)
        self.assertIn('name="image[]"; filename="reference.jpg"', body)
        self.assertIn("Content-Type: image/jpeg", body)

    def test_prompt_is_the_manifest_prompt_behind_one_owned_line(self):
        prompt = gen.build_prompt("Manifest prompt body.\n")
        self.assertTrue(prompt.startswith(gen.PROMPT_PREFIX))
        self.assertTrue(prompt.endswith("Manifest prompt body.\n"))

    def test_authorization_header_is_redacted_in_diagnostics(self):
        request = gen.build_generation_request(self.config, "PROMPT TEXT")
        described = gen.describe_request(request)
        self.assertNotIn(FAKE_KEY, described)
        self.assertIn("Bearer ***redacted***", described)


class DripResponse:
    """A peer that answers, slowly: one byte per chunk read, forever."""

    def __init__(self, body, chunk_delay=0.05, full_read_delay=2.0):
        self.body = body
        self.chunk_delay = chunk_delay
        self.full_read_delay = full_read_delay
        self.offset = 0

    def __enter__(self):
        return self

    def __exit__(self, *args):
        return False

    def read(self, size=-1):
        if size is None or size < 0:
            # A blocking full read: the whole body eventually arrives, long after
            # any per-socket inactivity timeout would have been satisfied.
            time.sleep(self.full_read_delay)
            self.offset = len(self.body)
            return self.body
        time.sleep(self.chunk_delay)
        chunk = self.body[self.offset : self.offset + 1]
        self.offset += len(chunk)
        return chunk


class DeadlineTests(unittest.TestCase):
    """The request budget is an end-to-end deadline, not a per-socket timeout."""

    def setUp(self):
        self.config = gen.resolve_config({"OPENAI_API_KEY": FAKE_KEY}, {})
        self.request = gen.build_generation_request(self.config, "PROMPT TEXT")

    def test_a_slow_drip_response_cannot_outlive_the_deadline(self):
        # Regression: 8541bf2 passed the timeout to urlopen only, where it is an
        # inactivity timeout, and then called response.read() with no deadline at
        # all — a peer trickling bytes held the sequential batch open indefinitely.
        body = json.dumps({"data": [{"b64_json": "AAAA"}]}).encode("utf-8")
        started = time.monotonic()
        with mock.patch.object(gen.urllib.request, "urlopen", return_value=DripResponse(body)):
            with self.assertRaises(RuntimeError) as caught:
                gen.post_with_retry(self.request, secret=FAKE_KEY, timeout=0.2)
        elapsed = time.monotonic() - started
        self.assertLess(elapsed, 1.5)
        self.assertIn("timed out", str(caught.exception).lower())
        self.assertNotIn(FAKE_KEY, str(caught.exception))

    def test_the_retry_backoff_counts_against_the_deadline(self):
        # Regression: 8541bf2 slept the full backoff even when the budget for this
        # image was already spent.
        failure = urllib.error.HTTPError(
            f"{self.config.base_url}/images/generations", 500, "Server Error", {}, io.BytesIO(b"boom")
        )
        started = time.monotonic()
        with mock.patch.object(gen.urllib.request, "urlopen", side_effect=failure):
            with self.assertRaises(RuntimeError):
                gen.post_with_retry(self.request, secret=FAKE_KEY, timeout=0.05)
        self.assertLess(time.monotonic() - started, gen.RETRY_BACKOFF_S)

    def test_a_prompt_response_is_read_in_full(self):
        payload = {"data": [{"b64_json": "AAAA"}], "usage": {"total_tokens": 7}}
        body = json.dumps(payload).encode("utf-8")
        response = DripResponse(body, chunk_delay=0.0, full_read_delay=0.0)
        with mock.patch.object(gen.urllib.request, "urlopen", return_value=response):
            self.assertEqual(gen.post_with_retry(self.request, secret=FAKE_KEY, timeout=30), payload)


class ConversionTests(unittest.TestCase):
    def test_png_is_converted_to_a_progressive_jpeg_within_the_max_width(self):
        jpeg = gen.png_to_jpeg(png_bytes(2400, 1600), max_width=1600, quality=85)
        with Image.open(io.BytesIO(jpeg)) as image:
            self.assertEqual(image.format, "JPEG")
            self.assertEqual(image.mode, "RGB")
            self.assertEqual(image.size, (1600, 1067))
            self.assertTrue(image.info.get("progressive"))

    def test_a_narrow_png_is_not_upscaled(self):
        jpeg = gen.png_to_jpeg(png_bytes(800, 600), max_width=1600, quality=85)
        with Image.open(io.BytesIO(jpeg)) as image:
            self.assertEqual(image.size, (800, 600))

    def test_lower_quality_produces_a_smaller_file(self):
        source = png_bytes(1200, 800)
        big = gen.png_to_jpeg(source, max_width=1600, quality=95)
        small = gen.png_to_jpeg(source, max_width=1600, quality=40)
        self.assertLess(len(small), len(big))


class AtomicWriteTests(unittest.TestCase):
    def test_replaces_the_target_and_leaves_no_temporary_behind(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "image.jpg"
            target.write_bytes(b"old")
            gen.write_atomic(target, b"new")
            self.assertEqual(target.read_bytes(), b"new")
            self.assertEqual([p.name for p in Path(tmp).iterdir()], ["image.jpg"])

    def test_stages_the_temporary_file_next_to_the_target(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "docs" / "images" / "image.jpg"
            target.parent.mkdir(parents=True)
            seen = []
            real_replace = os.replace

            def spy(src, dst):
                seen.append(Path(src).parent)
                real_replace(src, dst)

            with mock.patch.object(gen.os, "replace", spy):
                gen.write_atomic(target, b"payload")
            self.assertEqual(seen, [target.parent])

    def test_a_failed_write_leaves_the_previous_file_intact(self):
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "image.jpg"
            target.write_bytes(b"old")
            with mock.patch.object(gen.os, "replace", side_effect=OSError("boom")):
                with self.assertRaises(OSError):
                    gen.write_atomic(target, b"new")
            self.assertEqual(target.read_bytes(), b"old")
            self.assertEqual([p.name for p in Path(tmp).iterdir()], ["image.jpg"])


class ManifestEditTests(unittest.TestCase):
    """Targeted text edits on a copy of the real manifest — comments must survive."""

    def setUp(self):
        self.original = REAL_MANIFEST.read_text(encoding="utf-8")
        self.updated = gen.update_manifest_text(
            self.original,
            "coordination-architecture",
            generation_id="gen_abc123abc123",
            model="gpt-image-2",
            image_size="2048x1152",
            aspect_ratio="16:9",
            sha256="abc123abc123" + "0" * 52,
            updated_at="2026-08-28",
            history_comment="regenerated 2026-08-28 via openai gpt-image-2",
        )
        self.before = entry_blocks(self.original)
        self.after = entry_blocks(self.updated)

    def test_only_the_target_entry_changed(self):
        self.assertEqual(set(self.before), set(self.after))
        for key in self.before:
            if key == "coordination-architecture":
                self.assertNotEqual(self.before[key], self.after[key])
            else:
                self.assertEqual(self.before[key], self.after[key], f"{key} must be byte-identical")

    def test_header_comments_survive(self):
        self.assertEqual(self.before["__header__"], self.after["__header__"])
        self.assertIn("# Infographic generation manifest for taurhaus documentation.", self.updated)

    def test_prompt_and_reference_paths_are_untouched(self):
        block = self.after["coordination-architecture"]
        self.assertIn("Backend module map", block)
        self.assertIn("      reference_image_paths:\n        - docs/images/coordination-architecture.jpg\n", block)

    def test_stale_markers_are_removed(self):
        block = self.after["coordination-architecture"]
        self.assertNotIn("stale: true", block)
        self.assertNotIn("stale_since:", block)
        self.assertNotIn("stale_reason:", block)
        self.assertNotIn("sha256 describes the STALE", block)
        self.assertNotIn("STALE PIXELS", block)
        self.assertNotIn("Labels ClaudeNativeBackend", block)
        # Other stale entries keep their markers.
        self.assertIn("stale: true", self.after["daemon-protocol"])

    def test_recipe_and_checksum_fields_are_rewritten(self):
        block = self.after["coordination-architecture"]
        self.assertIn("    generation_id: gen_abc123abc123\n", block)
        self.assertIn("      model: gpt-image-2\n", block)
        self.assertIn('      image_size: "2048x1152"\n', block)
        self.assertIn('      aspect_ratio: "16:9"\n', block)
        self.assertIn("    sha256: abc123abc123" + "0" * 52 + "\n", block)
        self.assertIn('    updated_at: "2026-08-28"\n', block)

    def test_the_recorded_ratio_follows_the_geometry_that_made_the_image(self):
        # Regression: 8541bf2 rewrote image_size but left aspect_ratio alone, so a
        # 3:2 render kept a 16:9 recipe — a recipe that no longer makes the image.
        updated = gen.update_manifest_text(
            self.original,
            "coordination-architecture",
            generation_id="gen_abc123abc123",
            model="gpt-image-2",
            image_size="1536x1024",
            aspect_ratio="3:2",
            sha256="a" * 64,
            updated_at="2026-08-28",
            history_comment="regenerated 2026-08-28 via openai gpt-image-2",
        )
        block = entry_blocks(updated)["coordination-architecture"]
        self.assertIn('      image_size: "1536x1024"\n', block)
        self.assertIn('      aspect_ratio: "3:2"\n', block)
        self.assertNotIn('aspect_ratio: "16:9"', block)

    def test_history_gains_the_new_generation(self):
        block = self.after["coordination-architecture"]
        self.assertIn("      - gen_db88afd20f33  # v0.4.4 infographic regeneration\n", block)
        self.assertIn(
            "      - gen_abc123abc123  # regenerated 2026-08-28 via openai gpt-image-2\n",
            block,
        )

    def test_an_empty_history_list_becomes_a_real_list(self):
        updated = gen.update_manifest_text(
            self.original,
            "daemon-protocol",
            generation_id="gen_ffffffffffff",
            model="gpt-image-2",
            image_size="2048x1152",
            aspect_ratio="16:9",
            sha256="f" * 64,
            updated_at="2026-08-28",
            history_comment="regenerated 2026-08-28 via openai gpt-image-2",
        )
        block = entry_blocks(updated)["daemon-protocol"]
        self.assertNotIn("history: []", block)
        self.assertIn("    history:\n", block)
        self.assertIn(
            "      - gen_ffffffffffff  # regenerated 2026-08-28 via openai gpt-image-2\n",
            block,
        )

    def test_the_result_still_parses_as_yaml(self):
        parsed = gen.parse_manifest_text(self.updated)
        entry = parsed["images"]["coordination-architecture"]
        self.assertNotIn("stale", entry)
        self.assertEqual(entry["generation_id"], "gen_abc123abc123")
        self.assertEqual(entry["recipe"]["model"], "gpt-image-2")
        self.assertEqual(entry["recipe"]["image_size"], "2048x1152")
        self.assertEqual(entry["recipe"]["aspect_ratio"], "16:9")
        self.assertEqual(entry["updated_at"], "2026-08-28")
        self.assertEqual(len(entry["history"]), 2)


class DryRunTests(unittest.TestCase):
    def test_dry_run_prints_the_plan_and_writes_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp, env_text=f"OPENAI_API_KEY={FAKE_KEY}\n")
            before = (root / "docs" / "images" / "infographics.manifest.yaml").read_text(encoding="utf-8")
            buffer = io.StringIO()
            with mock.patch.object(gen.urllib.request, "urlopen", side_effect=AssertionError("network")):
                with redirect_stdout(buffer):
                    code = gen.main(["--dry-run", "--stale"], repo_root=root)
            output = buffer.getvalue()
            self.assertEqual(code, 0)
            self.assertIn("coordination-architecture", output)
            self.assertIn("gpt-image-2", output)
            self.assertIn("2048x1152", output)
            self.assertIn("high", output)
            self.assertIn("reference", output)
            self.assertIn("$", output)
            self.assertNotIn(FAKE_KEY, output)
            self.assertEqual(
                (root / "docs" / "images" / "infographics.manifest.yaml").read_text(encoding="utf-8"),
                before,
            )
            self.assertFalse((root / "docs" / "images" / ".generation-log.jsonl").exists())

    def test_price_override_changes_the_estimate(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp, env_text=f"OPENAI_API_KEY={FAKE_KEY}\n")
            buffer = io.StringIO()
            with redirect_stdout(buffer):
                code = gen.main(["--dry-run", "--id", "data-model", "--price-usd", "2.50"], repo_root=root)
            self.assertEqual(code, 0)
            self.assertIn("2.50", buffer.getvalue())


class RunTests(unittest.TestCase):
    def setUp(self):
        self.env_patch = mock.patch.dict(os.environ, {"OPENAI_API_KEY": FAKE_KEY}, clear=False)
        self.env_patch.start()
        self.addCleanup(self.env_patch.stop)
        self.backoff_patch = mock.patch.object(gen, "RETRY_BACKOFF_S", 0)
        self.backoff_patch.start()
        self.addCleanup(self.backoff_patch.stop)

    def test_a_successful_run_writes_the_jpeg_manifest_and_log(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp)
            with mock.patch.object(gen.urllib.request, "urlopen", side_effect=fake_urlopen()) as urlopen:
                buffer = io.StringIO()
                with redirect_stdout(buffer):
                    code = gen.main(["--id", "coordination-architecture"], repo_root=root)
            self.assertEqual(code, 0)

            request = urlopen.call_args.args[0]
            self.assertTrue(request.full_url.endswith("/images/edits"))

            image_path = root / "docs" / "images" / "coordination-architecture.jpg"
            with Image.open(image_path) as image:
                self.assertEqual(image.format, "JPEG")
                self.assertEqual(image.width, 1600)

            manifest_text = (root / "docs" / "images" / "infographics.manifest.yaml").read_text(encoding="utf-8")
            block = entry_blocks(manifest_text)["coordination-architecture"]
            self.assertNotIn("stale: true", block)
            digest = gen.sha256_hex(image_path.read_bytes())
            self.assertIn(f"    sha256: {digest}\n", block)
            self.assertIn(f"    generation_id: gen_{digest[:12]}\n", block)

            log_path = root / "docs" / "images" / ".generation-log.jsonl"
            record = json.loads(log_path.read_text(encoding="utf-8").strip())
            self.assertEqual(record["id"], "coordination-architecture")
            self.assertEqual(record["model"], "gpt-image-2")
            self.assertEqual(record["size"], "2048x1152")
            self.assertEqual(record["quality"], "high")
            self.assertEqual(record["sha256"], digest)
            self.assertIn("duration_s", record)
            self.assertNotIn(FAKE_KEY, log_path.read_text(encoding="utf-8"))

    def test_no_reference_forces_a_plain_generation(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp)
            with mock.patch.object(gen.urllib.request, "urlopen", side_effect=fake_urlopen()) as urlopen:
                with redirect_stdout(io.StringIO()):
                    code = gen.main(["--id", "coordination-architecture", "--no-reference"], repo_root=root)
            self.assertEqual(code, 0)
            request = urlopen.call_args.args[0]
            self.assertTrue(request.full_url.endswith("/images/generations"))

    def test_keep_png_writes_the_raw_generation_next_to_the_jpeg(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp)
            with mock.patch.object(gen.urllib.request, "urlopen", side_effect=fake_urlopen()):
                with redirect_stdout(io.StringIO()):
                    code = gen.main(["--id", "data-model", "--keep-png"], repo_root=root)
            self.assertEqual(code, 0)
            self.assertTrue((root / "docs" / "images" / "data-model.generated.png").exists())

    def test_a_server_error_is_retried_once_then_reported_without_the_key(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp)
            failure = urllib.error.HTTPError(
                "https://api.openai.com/v1/images/edits", 500, "Server Error", {}, io.BytesIO(b"upstream boom")
            )
            responses = [failure, failure, fake_response()]

            def urlopen(request, timeout=None):
                item = responses.pop(0)
                if isinstance(item, Exception):
                    raise item
                return item

            out, err = io.StringIO(), io.StringIO()
            with mock.patch.object(gen.urllib.request, "urlopen", side_effect=urlopen):
                with redirect_stdout(out), mock.patch.object(sys, "stderr", err):
                    code = gen.main(
                        ["--id", "coordination-architecture", "--id", "data-model"], repo_root=root
                    )

            self.assertNotEqual(code, 0)
            printed = out.getvalue() + err.getvalue()
            self.assertNotIn(FAKE_KEY, printed)
            self.assertIn("coordination-architecture", printed)
            self.assertIn("failed", printed.lower())
            # The second image still ran and landed.
            self.assertTrue((root / "docs" / "images" / "data-model.jpg").exists())
            manifest_text = (root / "docs" / "images" / "infographics.manifest.yaml").read_text(encoding="utf-8")
            blocks = entry_blocks(manifest_text)
            self.assertIn("stale: true", blocks["coordination-architecture"])
            self.assertNotIn("stale: true", blocks["data-model"])

    def test_a_size_that_contradicts_the_manifest_ratio_is_refused(self):
        # Regression: 8541bf2 — a 3:2 size against a 16:9 entry silently reshaped
        # the image and left the entry claiming 16:9.
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp)
            image_path = root / "docs" / "images" / "coordination-architecture.jpg"
            before_image = image_path.read_bytes()
            manifest = root / "docs" / "images" / "infographics.manifest.yaml"
            before_manifest = manifest.read_text(encoding="utf-8")
            out, err = io.StringIO(), io.StringIO()
            with mock.patch.dict(os.environ, {"OPENAI_IMAGE_SIZE": "1536x1024"}):
                with mock.patch.object(
                    gen.urllib.request, "urlopen", side_effect=AssertionError("network")
                ):
                    with redirect_stdout(out), mock.patch.object(sys, "stderr", err):
                        code = gen.main(["--id", "coordination-architecture"], repo_root=root)
            printed = out.getvalue() + err.getvalue()
            self.assertNotEqual(code, 0)
            self.assertIn("16:9", printed)
            self.assertIn("3:2", printed)
            self.assertIn("--allow-aspect-change", printed)
            self.assertEqual(image_path.read_bytes(), before_image)
            self.assertEqual(manifest.read_text(encoding="utf-8"), before_manifest)

    def test_allow_aspect_change_records_the_geometry_it_actually_used(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp)
            with mock.patch.dict(os.environ, {"OPENAI_IMAGE_SIZE": "1536x1024"}):
                with mock.patch.object(gen.urllib.request, "urlopen", side_effect=fake_urlopen()):
                    with redirect_stdout(io.StringIO()):
                        code = gen.main(
                            ["--id", "coordination-architecture", "--allow-aspect-change"],
                            repo_root=root,
                        )
            self.assertEqual(code, 0)
            manifest_text = (root / "docs" / "images" / "infographics.manifest.yaml").read_text(
                encoding="utf-8"
            )
            block = entry_blocks(manifest_text)["coordination-architecture"]
            self.assertIn('      image_size: "1536x1024"\n', block)
            self.assertIn('      aspect_ratio: "3:2"\n', block)

    def test_a_manifest_edit_failure_keeps_the_old_image_and_runs_the_next_entry(self):
        # Regression: 8541bf2 replaced the JPEG before computing the manifest edit and
        # left ManifestEditError out of the per-image catch, so a malformed entry shipped
        # new pixels under the old checksum *and* aborted the rest of the batch.
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp)
            image_path = root / "docs" / "images" / "coordination-architecture.jpg"
            before_image = image_path.read_bytes()
            manifest = root / "docs" / "images" / "infographics.manifest.yaml"

            real_update = gen.update_manifest_text

            def update(text, image_id, **kwargs):
                if image_id == "coordination-architecture":
                    raise gen.ManifestEditError(f"{image_id}: no history list in the entry")
                return real_update(text, image_id, **kwargs)

            out, err = io.StringIO(), io.StringIO()
            with mock.patch.object(gen, "update_manifest_text", side_effect=update):
                with mock.patch.object(gen.urllib.request, "urlopen", side_effect=fake_urlopen()):
                    with redirect_stdout(out), mock.patch.object(sys, "stderr", err):
                        code = gen.main(
                            ["--id", "coordination-architecture", "--id", "data-model"], repo_root=root
                        )

            printed = out.getvalue() + err.getvalue()
            self.assertNotEqual(code, 0)
            self.assertIn("coordination-architecture", printed)
            self.assertEqual(image_path.read_bytes(), before_image)
            blocks = entry_blocks(manifest.read_text(encoding="utf-8"))
            self.assertIn("stale: true", blocks["coordination-architecture"])
            # The next entry still ran.
            self.assertNotIn("stale: true", blocks["data-model"])
            self.assertNotEqual(
                (root / "docs" / "images" / "data-model.jpg").read_bytes(),
                before_image,
            )

    def test_a_failed_manifest_write_rolls_the_image_back(self):
        # Regression: 8541bf2 — the image was committed first, so a failed manifest
        # write left new pixels described by the old checksum and stale markers.
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp)
            image_path = root / "docs" / "images" / "coordination-architecture.jpg"
            before_image = image_path.read_bytes()
            manifest = root / "docs" / "images" / "infographics.manifest.yaml"
            before_manifest = manifest.read_text(encoding="utf-8")

            real_write = gen.write_atomic

            def write(path, data):
                if Path(path).name == "infographics.manifest.yaml":
                    raise OSError("read-only manifest")
                return real_write(path, data)

            out, err = io.StringIO(), io.StringIO()
            with mock.patch.object(gen, "write_atomic", side_effect=write):
                with mock.patch.object(gen.urllib.request, "urlopen", side_effect=fake_urlopen()):
                    with redirect_stdout(out), mock.patch.object(sys, "stderr", err):
                        code = gen.main(["--id", "coordination-architecture"], repo_root=root)

            self.assertNotEqual(code, 0)
            self.assertEqual(image_path.read_bytes(), before_image)
            self.assertEqual(manifest.read_text(encoding="utf-8"), before_manifest)

    def test_a_generation_log_failure_does_not_stop_the_run(self):
        # Regression: 8541bf2 appended to the log outside the per-image try, so a log
        # I/O error aborted every image after it.
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp)
            out, err = io.StringIO(), io.StringIO()
            with mock.patch.object(gen, "_log_record", side_effect=OSError("no space left")):
                with mock.patch.object(gen.urllib.request, "urlopen", side_effect=fake_urlopen()):
                    with redirect_stdout(out), mock.patch.object(sys, "stderr", err):
                        code = gen.main(
                            ["--id", "coordination-architecture", "--id", "data-model"], repo_root=root
                        )
            self.assertEqual(code, 0)
            blocks = entry_blocks(
                (root / "docs" / "images" / "infographics.manifest.yaml").read_text(encoding="utf-8")
            )
            self.assertNotIn("stale: true", blocks["coordination-architecture"])
            self.assertNotIn("stale: true", blocks["data-model"])
            self.assertIn("no space left", out.getvalue() + err.getvalue())

    def test_a_missing_api_key_fails_before_any_request(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = make_repo(tmp)
            err = io.StringIO()
            with mock.patch.dict(os.environ, {}, clear=True):
                with mock.patch.object(gen.urllib.request, "urlopen", side_effect=AssertionError("network")):
                    with mock.patch.object(sys, "stderr", err):
                        code = gen.main(["--id", "data-model"], repo_root=root)
            self.assertNotEqual(code, 0)
            self.assertIn("OPENAI_API_KEY", err.getvalue())


if __name__ == "__main__":
    unittest.main()
