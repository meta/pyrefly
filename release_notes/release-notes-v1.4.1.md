*Release date: November 4, 2026*

Pyrefly v1.4.1 is a patch release that fixes bugs in `pyrefly coverage`. With these fixes, you can use `pyrefly coverage check` as a pre-commit hook (`pyrefly-coverage-check` in [pyrefly-pre-commit](https://github.com/meta/pyrefly-pre-commit)).

---

## 🐛 Bug fixes

- **#5492:** `pyrefly coverage` now gives the same result on every run. Before, an attribute that inherits its annotation from a base class in another module could be reported as typed on one run and untyped on the next.
- **#5362:** `pyrefly coverage report` no longer panics when a pager or another program closes the output pipe early.
- **#5330:** `pyrefly coverage` no longer reports an excluded function or class (for example, one decorated with `@type_check_only` or `@no_type_check`) as an untyped variable when a module-level loop follows it.
- **#5383:** `pyrefly coverage` now counts a variable as typed when its value is an explicitly parameterized constructor call, such as `x = set[int]()`.

## ✨ Improvements

- **#5329:** `pyrefly coverage` understands Cython [pure Python mode](https://cython.readthedocs.io/en/latest/src/tutorial/pure.html) declarations in `.py` files.
- `pyrefly coverage` skips the solver for annotations that are absent or already solved, which makes it faster on large projects.

Thank-you to all our contributors who found these bugs and reported them! Did you know this is one of the most helpful contributions you can make to an open-source project? If you find any bugs in Pyrefly we want to know about them! Please open a bug report issue [here](https://github.com/facebook/pyrefly/issues).

---

## 📦 Upgrade

```bash
pip install --upgrade pyrefly==1.4.1
```

### How to safely upgrade your codebase

Upgrading the version of Pyrefly you're using or a third-party library you depend on can reveal new type errors in your code. Fixing them all at once is often unrealistic. We've written scripts to help you temporarily silence them. After upgrading, follow these steps:

1. `pyrefly check --suppress-errors`
2. Run your code formatter of choice
3. `pyrefly check --remove-unused-ignores`
4. Repeat until you achieve a clean formatting run and a clean type check.

This will add `# pyrefly: ignore` comments to your code, enabling you to silence errors and return to fix them later. This can make the process of upgrading a large codebase much more manageable.

Read more about error suppressions in the [Pyrefly documentation](https://pyrefly.org/en/docs/error-suppressions/).
