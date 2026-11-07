# < definition scip-python python snapshot-util 0.1 stdlib_backport/__init__:

# typing_extensions is in the bundled stdlib stubs, but an installed distribution takes
# precedence.
from typing_extensions import Doc
#    ^^^^^^^^^^^^^^^^^ reference  typing_extensions 4.16.0 typing_extensions/__init__:
#                             ^^^ reference  typing_extensions 4.16.0 typing_extensions/Doc#

print(Doc)
#^^^^ reference  python-stdlib 3.13 builtins/print().
#     ^^^ reference  typing_extensions 4.16.0 typing_extensions/Doc#

