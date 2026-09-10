---
on:
  schedule:
    # Fuzzy schedule — GitHub distributes the exact run time within the
    # window rather than everyone's cron firing at the same instant.
    - cron: "weekly on monday"
  workflow_dispatch:

engine: claude

permissions:
  contents: read
  issues: read

safe-outputs:
  add-comment:
    max: 20
  add-labels:
    max: 20
  create-issue:
    max: 1

timeout-minutes: 15
---

# Weekly Issue Triage

Review every open issue in this repository that does not yet have a
priority label (`priority:high`, `priority:medium`, `priority:low`).

For each untriaged issue:

1. Read the issue body and any existing comments.
2. Assess whether it's actionable as written — a bug report needs
   reproduction steps, a feature request needs a clear description of
   the desired behavior.
3. If it's actionable, add one priority label based on impact and
   apply the most fitting label from: `bug`, `enhancement`,
   `documentation`, `question` — whichever the content actually
   matches, don't force one if none fit.
4. If it's NOT actionable — missing reproduction steps, unclear ask,
   duplicate of another open issue — leave a comment explaining
   specifically what's missing, and apply the `needs-info` label.
5. Do not close anything. Do not assign anyone. Labeling and one
   comment per issue only.

At the end, create one new issue titled "Weekly Triage Summary — <date>"
summarizing: how many issues were reviewed, how many were actionable,
how many needed more info, and the single oldest untriaged issue if
any remain after this pass.
