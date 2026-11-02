# < definition scip-python python snapshot-util 0.1 main/__init__:

# BUG: These lambda-local bindings share a module-level symbol.
# Each `hidden` should have a distinct local symbol.
first = lambda: (hidden := 1)
#^^^^ definition  snapshot-util 0.1 main/first.
#                ^^^^^^ definition  snapshot-util 0.1 main/hidden.
second = lambda: (hidden := 2)
#^^^^^ definition  snapshot-util 0.1 main/second.
#                 ^^^^^^ definition  snapshot-util 0.1 main/hidden.

