#!/bin/bash
# ===========================================================================
# Load test API keys into the current shell session (memory only).
#
# Usage (on the Mac, after scp'ing .maccreds):
#   source ~/load-test-keys.sh
#
# Keys live in shell memory only. They vanish when the session ends.
# Do NOT add this to any shell profile.
# ===========================================================================

CREDS_FILE="$HOME/.maccreds"

if [[ ! -f "$CREDS_FILE" ]]; then
    echo "Error: $CREDS_FILE not found"
    echo "scp it from your dev machine first:"
    echo "  scp .maccreds m1@<IP>:~/"
    return 1 2>/dev/null || exit 1
fi

# Parse key=value format
while IFS='=' read -r key value; do
    [[ -z "$key" || "$key" =~ ^# ]] && continue
    case "$key" in
        antropic|anthropic)
            export ANTHROPIC_API_KEY="$value"
            echo "ANTHROPIC_API_KEY set (${#value} chars)"
            ;;
        openai)
            export OPENAI_API_KEY="$value"
            echo "OPENAI_API_KEY set (${#value} chars)"
            ;;
        google|gemini)
            # Antigravity's enterprise/Vertex path only; the retired Gemini
            # CLI is gone and agy normally signs in through the Google flow.
            export GEMINI_API_KEY="$value"
            echo "GEMINI_API_KEY set (${#value} chars)"
            ;;
        xai|grok)
            export XAI_API_KEY="$value"
            echo "XAI_API_KEY set (${#value} chars)"
            ;;
        *)
            echo "Unknown key: $key (skipped)"
            ;;
    esac
done < "$CREDS_FILE"

echo ""
echo "Keys loaded into this session. They will vanish when you exit."
echo "After testing, delete the creds file: rm ~/.maccreds"
