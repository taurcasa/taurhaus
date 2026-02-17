#!/usr/bin/env bash
# Session handoff hook — creates structured handoff files when a Claude Code
# session ends. Called by the SessionEnd hook in .claude/settings.json.
#
# Receives JSON on stdin: { session_id, transcript_path, cwd, reason, ... }
# Writes two files to docs/sessions/:
#   1. session-YYYY-MM-DDTHH-MM-SS.md   (YAML frontmatter + notes)
#   2. session-YYYY-MM-DDTHH-MM-SS.meta.json  (session metadata)
#
# Requires: jq, claude CLI (for transcript summarization)

set -euo pipefail

# Read hook input from stdin
INPUT=$(cat)

SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // "unknown"')
TRANSCRIPT_PATH=$(echo "$INPUT" | jq -r '.transcript_path // ""')
CWD=$(echo "$INPUT" | jq -r '.cwd // "."')
REASON=$(echo "$INPUT" | jq -r '.reason // "other"')

# Derive project name from working directory
PROJECT_NAME=$(basename "$CWD")

# Generate timestamp for filenames
TIMESTAMP=$(date -u +"%Y-%m-%dT%H-%M-%S")
ISO_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Ensure output directory exists
SESSIONS_DIR="$CWD/docs/sessions"
mkdir -p "$SESSIONS_DIR"

HANDOFF_FILE="$SESSIONS_DIR/session-${TIMESTAMP}.md"
META_FILE="$SESSIONS_DIR/session-${TIMESTAMP}.meta.json"

# If no transcript or file doesn't exist, write minimal handoff
if [ -z "$TRANSCRIPT_PATH" ] || [ ! -f "$TRANSCRIPT_PATH" ]; then
    cat > "$HANDOFF_FILE" << ENDOFHANDOFF
---
date: ${ISO_DATE}
project: ${PROJECT_NAME}
session_id: ${SESSION_ID}
summary: >
  Session ended without transcript data available.
next_steps: []
open_questions: []
metadata:
  exit_reason: ${REASON}
---

## Session Notes

No transcript was available for this session.
ENDOFHANDOFF

    cat > "$META_FILE" << ENDOFMETA
{
  "session_id": "${SESSION_ID}",
  "ended_at": "${ISO_DATE}",
  "exit_reason": "${REASON}"
}
ENDOFMETA
    exit 0
fi

# Use Claude (Haiku) to summarize the transcript and generate handoff content.
# The prompt instructs the model to output structured YAML-compatible text.
SUMMARY_PROMPT="You are a session handoff summarizer. Read the following Claude Code session transcript and output EXACTLY this format with no extra text:

SUMMARY: <2-3 sentence summary of what was accomplished>
NEXT_STEPS:
- <specific actionable step 1>
- <specific actionable step 2>
- <specific actionable step 3>
OPEN_QUESTIONS:
- <any unresolved question, or NONE if no open questions>
NOTES: <1-2 paragraph narrative of key decisions, changes made, and important context>

Rules:
- Keep summary concise (2-3 sentences)
- Next steps must be specific and actionable (3-5 items)
- Open questions only if genuinely unresolved
- Notes should capture key context someone needs to continue this work
- If the transcript is very short, still provide what you can determine"

# Extract a reasonable portion of the transcript (first and last parts if large)
TRANSCRIPT_SIZE=$(wc -c < "$TRANSCRIPT_PATH" 2>/dev/null || echo "0")
MAX_CHARS=100000

if [ "$TRANSCRIPT_SIZE" -gt "$MAX_CHARS" ]; then
    # For large transcripts, take first 40K and last 60K chars
    TRANSCRIPT_CONTENT="[Transcript truncated - showing first and last portions]\n\n--- START ---\n$(head -c 40000 "$TRANSCRIPT_PATH")\n\n--- GAP (middle omitted) ---\n\n--- END ---\n$(tail -c 60000 "$TRANSCRIPT_PATH")"
else
    TRANSCRIPT_CONTENT=$(cat "$TRANSCRIPT_PATH")
fi

# Call Claude CLI to summarize (Haiku for speed and cost)
SUMMARY_OUTPUT=$(echo "$TRANSCRIPT_CONTENT" | claude --model claude-haiku-4-5-20251001 \
    --print --no-input \
    --prompt "$SUMMARY_PROMPT" \
    2>/dev/null || echo "SUMMARY: Session ended. Unable to generate summary from transcript.
NEXT_STEPS:
- Review the session transcript manually
OPEN_QUESTIONS:
- NONE
NOTES: Automatic summarization failed. Check the transcript at ${TRANSCRIPT_PATH} for details.")

# Parse the structured output
SUMMARY=$(echo "$SUMMARY_OUTPUT" | sed -n 's/^SUMMARY: *//p' | head -1)
NOTES=$(echo "$SUMMARY_OUTPUT" | sed -n '/^NOTES: */,$ { s/^NOTES: *//; p; }')

# Extract next steps (lines starting with - after NEXT_STEPS:)
NEXT_STEPS_YAML=$(echo "$SUMMARY_OUTPUT" | sed -n '/^NEXT_STEPS:/,/^[A-Z_]*:/{/^NEXT_STEPS:/d; /^[A-Z_]*:/d; p}' | sed 's/^/  /')

# Extract open questions
OPEN_QUESTIONS_YAML=$(echo "$SUMMARY_OUTPUT" | sed -n '/^OPEN_QUESTIONS:/,/^[A-Z_]*:/{/^OPEN_QUESTIONS:/d; /^[A-Z_]*:/d; p}' | sed 's/^/  /')

# Check if open questions is just NONE
if echo "$OPEN_QUESTIONS_YAML" | grep -qi "none"; then
    OPEN_QUESTIONS_YAML="  []"
fi

# Fallback values
[ -z "$SUMMARY" ] && SUMMARY="Session ended. Summary not available."
[ -z "$NEXT_STEPS_YAML" ] && NEXT_STEPS_YAML="  - Review session transcript"
[ -z "$NOTES" ] && NOTES="No additional notes."

# Write the handoff markdown
cat > "$HANDOFF_FILE" << ENDOFHANDOFF
---
date: ${ISO_DATE}
project: ${PROJECT_NAME}
session_id: ${SESSION_ID}
summary: >
  ${SUMMARY}
next_steps:
${NEXT_STEPS_YAML}
open_questions:
${OPEN_QUESTIONS_YAML}
metadata:
  exit_reason: ${REASON}
---

## Session Notes

${NOTES}
ENDOFHANDOFF

# Write the metadata sidecar
cat > "$META_FILE" << ENDOFMETA
{
  "session_id": "${SESSION_ID}",
  "ended_at": "${ISO_DATE}",
  "exit_reason": "${REASON}",
  "model": "unknown",
  "tools_used": {},
  "files_modified": [],
  "tokens": {}
}
ENDOFMETA

exit 0
