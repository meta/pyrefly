# < definition scip-python python snapshot-util 0.1 vars_inside_scopes/__init__:

from typing import List
#    ^^^^^^ reference  python-stdlib 3.13 typing/__init__:
#                  ^^^^ reference  python-stdlib 3.13 typing/List.

class X:
#     ^ definition  snapshot-util 0.1 vars_inside_scopes/X#
    items: List[int]
#   ^^^^^ definition  snapshot-util 0.1 vars_inside_scopes/X#items.
#          ^^^^ reference  python-stdlib 3.13 typing/List.
#               ^^^ reference  python-stdlib 3.13 builtins/int#

    def my_func(self):
#       ^^^^^^^ definition  snapshot-util 0.1 vars_inside_scopes/X#my_func().
#               ^^^^ definition  snapshot-util 0.1 vars_inside_scopes/X#my_func().(self)
        for x in self.items:
#           ^ definition local 0
#                ^^^^ reference  snapshot-util 0.1 vars_inside_scopes/X#my_func().(self)
#                     ^^^^^ reference  snapshot-util 0.1 vars_inside_scopes/X#items.
            y = x + 1
#           ^ definition local 1
#               ^ reference local 0

        if 5 in self.items:
#               ^^^^ reference  snapshot-util 0.1 vars_inside_scopes/X#my_func().(self)
#                    ^^^^^ reference  snapshot-util 0.1 vars_inside_scopes/X#items.
            z = "oh ya"
#           ^ definition local 2

