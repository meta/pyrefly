---
name: score-pyrefly-change
description: Produces a scorecard evaluating a pyrefly change on correctness and quality.
disable-model-invocation: true
---

Evaluate this change, executing the following checklist in this exact order.

## Review principles

- Prefer simple, consistent static typing rules over increasingly precise simulation of runtime behavior. Consistency improves user predictability and limits implementation complexity. Typing rules still apply inside `TYPE_CHECKING` blocks unless the specification or intended support contract requires different behavior. Runtime success alone does not show that a diagnostic is incorrect.
- Justify exceptions by their practical benefit, conceptual cost, and maintenance burden. Finding an uncommon counterexample does not require supporting it. Distinguish a violated contract from a reasonable support boundary or an unresolved semantic choice.

## 1. Explain the change

Record the exact change you are evaluating, and explain what problem it solves and how in plain language. Briefly describe the core building blocks, using full relative paths.

## 2. Fill out the scorecard

The scorecard consists of weighted sections with concrete dimensions. For each dimension, fill out:
* A verdict of YES, NO, UNKNOWN, or N/A.
* Evidence for a YES or NO verdict. For UNKNOWN, describe what data you're missing. Score N/A only when a dimension is structurally irrelevant, such as Code Quality for a documentation-only change.

### Design (40%)

- **D1**: Does the change address a real problem in Pyrefly?
  - A bug fix requires evidence that Pyrefly's behavior conflicts with the typing specification, Python runtime semantics, the LSP specification, or established behavior in other tools.
  - A new feature requires evidence that the feature is in-scope for Pyrefly, such as a linked GitHub issue created or confirmed by a maintainer.
  - Judge other changes, such as code refactors and documentation updates, on whether they improve the project on balance.
  - Never score this dimension as N/A.

If the problem cannot be demonstrated by a reproducer, then score N/A for the remaining design dimensions.

Otherwise, using D1's evidence, develop a minimal neutral reproducer for the problem that does not include the author's characterization or desired mechanism. Give this reproducer and the change's base revision to two fresh-context sub-agents, along with these restrictions:
- Do not switch or modify the shared working copy.
- Inspect only the base revision, using read-only revision-aware commands.
- Do not read the change's metadata or contents.
Ask each sub-agent to determine:
1. The underlying logic bug, design flaw, or missing piece that causes the problem - i.e., the "root cause".
2. What the correct behavior should be on the minimal reproducer and 1 additional representative example for the root cause.
3. The core idea that a principled fix should use, independent of specific implementation strategy.
4. Where in the type checker/language server life cycle the fix should live.
Give each sub-agent the same prompt, including the review principles above, and record that exact prompt. Ask them to identify the specification or intended support contract that justifies their expected behavior, and state uncertainty when it is unclear.

Map the sub-agents' responses to 1-4 to D2-D5, respectively. Score UNKNOWN for any design dimension on which the sub-agents disagree.

- **D2**: Does the change target the root cause?
- **D3**: Does the change produce the correct behavior on the minimal reproducer?
  - Ignore the additional representative examples.
- **D4**: Does the change use the correct core idea?
- **D5**: Is the change implemented at the correct point(s) in the type checker/language server life cycle?

### Implementation (30%)

If the change modifies code, review the change adversarially.
- Do not assume the description, tests, or apparent local correctness prove the approach.
- Identify the change's central invariant and its basis in the specification or intended support contract, derive a matrix of expected behaviors, and actively try to falsify it with minimal counterexamples.
- If sub-agents were used, include their additional representative examples here.
Record all counterexamples discovered.

Always score I1 and I2. For non-code changes, interpret I1 as whether the changed content is accurate and internally consistent.

- **I1**: Does the change implement its chosen semantics correctly?
  - Consider only counterexamples that demonstrate an incorrect behavior or side effect of changed or added code.
  - Distinguish incorrect from imprecise: score YES if the change faithfully implements what it claims to, even if more precise semantics are possible.
  - If the intended semantics are unclear, score UNKNOWN rather than assuming a counterexample violates them.
- **I2**: Does the change fully implement its intended scope?
  - Consider only counterexamples that demonstrate an unintentionally incomplete implementation of the change description.
- **I3**: Is the change's mypy_primer delta free of regressions?
  - If a mypy_primer delta is present as a pull request comment or diff signal, analyze it using steps 2-4 of the `interpret-mypy-primer` skill.

### Validation (10%)

- **V1**: Is the change's core functionality validated?
  - Automated tests are preferable, but manual validation documented in the change description is acceptable when automated testing is infeasible.
- **V2**: Is coverage robust?
  - Score YES if a majority of changed and new code paths are validated.
  - Exhaustive edge case coverage is not required.

### Code Quality (10%)

- **C1**: Is the control flow easy to follow?
- **C2**: Does the code use existing abstractions and helpers instead of reinventing them or duplicating code?
- **C3**: Does the code make appropriate use of concise, readable comments to explain non-obvious aspects?
  - Code without comments can score YES if the code does not need comments.
  - 2+ unnecessary or poorly written comments should score NO.

### Reviewability (10%)

- **R1**: Is the change focused?
  - Score NO if the change does multiple things that could be cleanly separated into self-contained changes.
- **R2**: Is the change small?
  - Score YES if the change is split into pieces - for example, commits in a large PR - that each satisfy this dimension.
  - Each piece should be no more than approximately 150 LOC, not including tests.
- **R3**: Does the change have a clear, informative title?
- **R4**: Does the change have a clear, informative description?
  - An unnecessarily verbose description, such as raw LLM output, should score NO.

## 3. Summarize

Use scripts/score.py to compute per-section and overall quality scores. Report the scores, and briefly describe what the change does well and what can be improved.

Separately classify findings as blocking, non-blocking, or unresolved. A counterexample, a NO score, or agreement between reviewers does not automatically justify blocking. For each blocker, explain the violated specification, intended support contract, or architectural requirement, its concrete user or maintenance impact, and why correction is required in this change rather than documentation or follow-up. Present semantic uncertainty and optional precision improvements as non-blocking or unresolved, not as established correctness blockers.

## 4. Present

Present your findings using assets/template.md. Do not modify your findings while compiling them for presentation.
