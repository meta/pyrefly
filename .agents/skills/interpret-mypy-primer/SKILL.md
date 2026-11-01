---
name: interpret-mypy-primer
description: Interprets a pyrefly mypy_primer error differential. Use when a pyrefly change alters which errors are reported (a new or retyped diagnostic, a new error kind, or an inference change). Classifies added and removed errors as true positives or negatives (improvements) vs false positives or negatives (regressions) and summarizes the result.
---

# Interpreting a pyrefly mypy_primer differential

[mypy_primer](https://github.com/hauntsaninja/mypy_primer) runs a type checker over a
large corpus of real open-source projects and reports a **differential** between two
revisions: the errors a change newly adds (`+`) and the errors it removes (`-`),
per project. Pyrefly's CI runs this on pull requests and posts the differential as a
comment.

This skill turns a raw differential into a verdict and a short write-up.

Core principle: **do not assert, verify.** Counts alone are insufficient to interpret a
differential. Sample added/removed errors, reproduce them minimally, and use reference
checkers and the Python runtime to determine correct behavior — do not eyeball the
message and guess.

## 1. Get the differential

- From CI: read the mypy_primer comment on the pull request.
- Locally: run `mypy_primer` comparing your branch against its base, or build pyrefly
  at both revisions with `cargo build --release` and run each on the same fixed
  checkout of a few corpus projects (`pyrefly check --preset <p> <project>`), then diff
  the error lists. Measuring on fixed code is essential — totals are not comparable when
  a corpus project's HEAD moves between runs.

## 2. Interpret by preset

Report the differential as `+added / -removed`, broken down by the preset(s) you ran.

- The shipping presets (`default`, `strict`) matter most: a change should ideally be
  `+0/-0` there, or add/remove only true positives/negatives. New errors in `default` are
  seen by every user, so they carry the highest bar.
- A large volume under the `all` preset is expected and acceptable for a new opt-in
  error kind (one that defaults to `ignore` and only reports under `all`). Say so rather
  than treating the number as alarming.
- Confirm the additions are the kind you intended. If a change meant to add
  `foo-error` also moves counts for unrelated kinds, investigate.

## 3. Discount noise

- If the same error, with identical message and location, appears as both added and
  removed, this is noise from errors being emitted in a different order. Discount it and
  say so.
- If the same error appears as added and removed at the same location with different
  messages, this is an error message change. Review message quality separately, discount
  the error from correctness verification, and say so.

## 4. Verify correctness

Sample the differential — at least the largest clusters, and a spread of kinds. For each
sampled entry:

1. Build a minimal reproducer that triggers the same error, without copying the
   change's intended behavior into it.
2. Run it through pyrefly at your change, a reference checker (mypy and/or
   pyright — install via `uv run --with mypy python -m mypy <file>` /
   `uv run --with pyright pyright <file>`), and, where the behavior is observable, the
   CPython runtime.

Classify errors:
- An added error that is correct is a **true positive** (improvement); an incorrect one
  is a **false positive** (regression).
- A removed error that was incorrect leaves a **true negative** (improvement); one that
  was correct leaves a **false negative** (regression).

Weigh the evidence:
- The typing specification (https://typing.python.org/en/latest/spec/) decides what a
  type checker should report. When it covers a scenario unambiguously, it overrides
  everything below.
- CPython failing at an operation for the reason that an error states shows that the code
  is wrong, providing strong evidence for a true positive. CPython succeeding is weak
  evidence: a program that merely runs is not proof it is type-correct.
- A reference checker agreeing with pyrefly is strong evidence for an improvement.
  Disagreement does not prove a regression, as pyrefly sometimes intentionally differs.
- Flag errors with weak or contradictory evidence for maintainer review.

Attribute errors by before/after build, not by editing the program: run the same
reproducer through pyrefly built at the base revision and at your change. An error only
on your build and not on the base revision or vice versa is caused by the change; an
error on both pre-exists.

Patterns worth knowing: a single root cause often explains a whole cluster of per-project
errors — fix the cause, not each symptom, and re-run. Framework classes
(`dataclass_transform`, metaclasses, `Annotated`) are a common false-positive source and
need the real framework in the reproducer, not a plain stand-in.

## 5. Summarize the result

Summarize the result in a short paragraph stating:

- the per-preset deltas (`+X / -Y`), and that the additions are `<kind>`;
- the shipping-preset (`default`/`strict`) impact specifically;
- any noise you discounted and why;
- a one-line correctness verdict from your sampling — e.g. "sampled the 40 additions,
  all true positives (mypy agrees)", or "~95% concordant with pyright; the rest are
  known precision gaps".

The goal is that a reviewer can see the real-world impact of the change without re-running
anything.

## Inside Meta (internal checkout)

The interpretation, noise handling, and verification above are identical; only the plumbing differs.

- Get the differential from CI with the `ci-signals` skill: the `pyrefly-mypy-primer`
  signal (plus the `instagram-server` and `pytorch` signals) carries it. Build before/after
  with `buck2 build @fbcode//mode/opt fbcode//pyrefly:pyrefly` (copy the binary out
  immediately — the output path is reused across builds) rather than cargo.
