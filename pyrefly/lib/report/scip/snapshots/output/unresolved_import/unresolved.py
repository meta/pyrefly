# < definition scip-python python snapshot-util 0.1 unresolved/__init__:

import this_is_not_real
#      ^^^^^^^^^^^^^^^^ reference local 0

print(this_is_not_real.x)
#^^^^ reference  python-stdlib 3.13 builtins/print().
#     ^^^^^^^^^^^^^^^^ reference local 0
print(this_is_not_real.x)
#^^^^ reference  python-stdlib 3.13 builtins/print().
#     ^^^^^^^^^^^^^^^^ reference local 0


