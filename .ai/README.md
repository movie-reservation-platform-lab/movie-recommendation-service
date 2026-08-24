# AI Guidance Source

This directory is the source of truth for repository-specific AI guidance.

## Structure

- `project-guidance.md`: always-on repository context and safety boundaries.
- `skills/*/SKILL.md`: reusable workflows loaded when their descriptions match.
- `skills/*/agents/openai.yaml`: Codex skill-list metadata where provided.
- `agents/*.md`: read-only review-agent definitions.
- `meta/*.yaml`: generated-index descriptions and skill names.
- `sync.sh`: publishes canonical content to supported assistant directories.

## Workflow

1. Edit canonical content under `.ai/`.
2. Keep skill frontmatter and matching `.ai/meta/*.yaml` aligned.
3. Run `bash .ai/sync.sh`.
4. Review both canonical and generated diffs.
5. Validate changed skills and run `git diff --check`.

Do not edit generated `.codex`, `.claude`, `.cursor`, `.gemini`, `.roo`, or root
guidance directly. Add repository-specific behavior to canonical guidance rather
than changing a copied skill in only one generated location.
