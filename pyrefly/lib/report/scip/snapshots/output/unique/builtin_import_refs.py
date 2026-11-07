# < definition scip-python python snapshot-util 0.1 builtin_import_refs/__init__:

from typing import Any
#    ^^^^^^ reference  python-stdlib 3.13 typing/__init__:
#                  ^^^ reference  python-stdlib 3.13 typing/Any.

print(Any)
#^^^^ reference  python-stdlib 3.13 builtins/print().
#     ^^^ reference  python-stdlib 3.13 typing/Any.

