# < definition scip-python python snapshot-util 0.1 file_from_module/__init__:

from xyz import nested_file
#    ^^^ reference  snapshot-util 0.1 xyz/__init__:
#               ^^^^^^^^^^^ reference  snapshot-util 0.1 `xyz.nested_file`/__init__:

print(nested_file.X)
#^^^^ reference  python-stdlib 3.13 builtins/print().
#     ^^^^^^^^^^^ reference  snapshot-util 0.1 `xyz.nested_file`/__init__:
#                 ^ reference  snapshot-util 0.1 `xyz.nested_file`/X.

