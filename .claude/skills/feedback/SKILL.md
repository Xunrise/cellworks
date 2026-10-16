---
name: feedback
description: Capture an off-topic finding as a GitHub issue without leaving the current work, or triage the inbox of captured findings. Use for "/feedback <text>" and "/feedback triage".
allowed-tools: Bash, Read, Skill
---

# Feedback inbox

Findings made while testing a story that do not belong to it go to an inbox of issues labelled `feedback`, so the current session stays on its story.

## `/feedback <text>`

Capture and stop. Do not investigate, fix, or discuss it.

1. Collect: the text as given, the current branch (`git branch --show-current`), and the story under test: the `STORY-NNN` or `Intent: #N` in the branch's latest commits or its pull request (`gh pr view --json title,body`); `none` when there is none.
2. Create the issue, title = the first line of the text, truncated to 80 characters:

   ```
   gh issue create --label feedback --title "<title>" --body "<body>"
   ```

   Body:

   ```
   <text>

   - Branch: `<branch>`
   - Story under test: <STORY-NNN or none>
   ```
3. Reply with the issue URL and nothing else. Return to the previous task.

## `/feedback triage`

Walk the inbox one issue at a time.

1. `gh issue list --label feedback --state open --json number,title,body,createdAt`, oldest first. If it is empty, say so and stop.
2. For each issue, show its title and body and propose one outcome, then wait for the person's choice:
   - **drop**: comment the reason, remove nothing else, `gh issue close <n> --reason "not planned"`.
   - **fold into an issue**: ask which one (`gh issue list --search` can help); add the finding as a comment there, then close the feedback issue with a link to it.
   - **promote**: rewrite it as a story. A change in behavior goes through `gherkin-story-authoring`, anything else through `chore-story`. When the person approves the story, remove the `feedback` label from the original and close it with a link to the story; label the new story `sdlc:intent` only when the person says to start the pipeline.
3. Never change an issue without the person's choice for it. After the last one, print a one-line count of each outcome.
