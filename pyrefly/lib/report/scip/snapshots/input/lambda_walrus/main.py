# BUG: These lambda-local bindings share a module-level symbol.
# Each `hidden` should have a distinct local symbol.
first = lambda: (hidden := 1)
second = lambda: (hidden := 2)
