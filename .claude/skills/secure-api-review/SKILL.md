---
name: secure-api-review
description: >-
  Apply the API security standard. Use whenever creating or modifying an
  external-facing endpoint, reviewing API code, designing a request or response
  schema, or generating an OpenAPI spec.
allowed-tools: Read, Glob, Grep, Bash
---

# Secure API review

> **This is a worked example of a *policy* skill**, as distinct from the
> factory plugin's stage skills. It shows the shape institutional knowledge
> takes here: narrow trigger, numbered rules, no room for interpretation.
> Replace the rules below with your real standard, and have the policy owner
> sign off the change. See the note at the bottom on what a skill can and
> cannot enforce.

When you create or change an API endpoint:

1. **Authentication** — every endpoint requires the gateway JWT. No anonymous
   routes outside `/health`.
2. **Input validation** — validate request bodies against the OpenAPI schema and
   reject unknown fields. Reject, do not ignore: silently dropping an unknown
   field hides a client that thinks it sent something.
3. **Audit** — every state-changing endpoint emits an audit event carrying actor,
   action, entity, and timestamp.
4. **Data classification** — a field tagged `pii` in the schema must never appear
   in a log line or an error message. Project it away at the boundary rather than
   filtering at each log site, so the rule holds by construction.
5. **Rate limits** — a new consumer of an internal service states the limit it
   will respect and caches accordingly. Check the limit before designing the
   call pattern, not after.

## A skill is advisory

This file makes the policy likely to be applied while code is written. It cannot
guarantee it — nothing forces a session to comply.

A rule that must hold without exception needs something deterministic behind it:
a hook that blocks the action, a CI check, or a review pass that re-checks it at
the PR. The skill makes violations rare; the hook makes them close to
impossible. Rules 1 and 4 above are the kind that warrant one.
