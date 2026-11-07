# < definition scip-python python snapshot-util 0.1 fstring/__init__:

var = ", world!"
#^^ definition  snapshot-util 0.1 fstring/var.

print(f"var: hello {var}")
#^^^^ reference  python-stdlib 3.13 builtins/print().
#                   ^^^ reference  snapshot-util 0.1 fstring/var.

