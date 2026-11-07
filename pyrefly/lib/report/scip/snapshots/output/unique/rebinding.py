# < definition scip-python python snapshot-util 0.1 rebinding/__init__:

# Only the first binding of a symbol is its definition, except that every overload defines its
# parameters.
from typing import overload
#    ^^^^^^ reference  python-stdlib 3.13 typing/__init__:
#                  ^^^^^^^^ reference  python-stdlib 3.13 typing/overload().

x = 1
# definition  snapshot-util 0.1 rebinding/x.
x = 2
# reference  snapshot-util 0.1 rebinding/x.


def f():
#   ^ definition  snapshot-util 0.1 rebinding/f().
    y = 1
#   ^ definition local 0
    y = 2
#   ^ reference local 0
    return y
#          ^ reference local 0


class C:
#     ^ definition  snapshot-util 0.1 rebinding/C#
    a = 1
#   ^ definition  snapshot-util 0.1 rebinding/C#a.
    a = 2
#   ^ reference  snapshot-util 0.1 rebinding/C#a.

    def __init__(self):
#       ^^^^^^^^ definition  snapshot-util 0.1 rebinding/C#__init__().
#                ^^^^ definition  snapshot-util 0.1 rebinding/C#__init__().(self)
        self.b = 1
#       ^^^^ reference  snapshot-util 0.1 rebinding/C#__init__().(self)
#            ^ definition  snapshot-util 0.1 rebinding/C#b.
        self.b = 2
#       ^^^^ reference  snapshot-util 0.1 rebinding/C#__init__().(self)
#            ^ reference  snapshot-util 0.1 rebinding/C#b.

    @overload
#    ^^^^^^^^ reference  python-stdlib 3.13 typing/overload().
    def m(self, v: int) -> int: ...
#       ^ definition  snapshot-util 0.1 rebinding/C#m().
#         ^^^^ definition  snapshot-util 0.1 rebinding/C#m().(self)
#               ^ definition  snapshot-util 0.1 rebinding/C#m().(v)
#                  ^^^ reference  python-stdlib 3.13 builtins/int#
#                          ^^^ reference  python-stdlib 3.13 builtins/int#
    @overload
#    ^^^^^^^^ reference  python-stdlib 3.13 typing/overload().
    def m(self, v: str) -> str: ...
#       ^ reference  snapshot-util 0.1 rebinding/C#m().
#         ^^^^ definition  snapshot-util 0.1 rebinding/C#m().(self)
#               ^ definition  snapshot-util 0.1 rebinding/C#m().(v)
#                  ^^^ reference  python-stdlib 3.13 builtins/str#
#                          ^^^ reference  python-stdlib 3.13 builtins/str#
    def m(self, v):
#       ^ reference  snapshot-util 0.1 rebinding/C#m().
#         ^^^^ definition  snapshot-util 0.1 rebinding/C#m().(self)
#               ^ definition  snapshot-util 0.1 rebinding/C#m().(v)
        return v
#              ^ reference  snapshot-util 0.1 rebinding/C#m().(v)


def g(): ...
#   ^ definition  snapshot-util 0.1 rebinding/g().
def g(): ...
#   ^ reference  snapshot-util 0.1 rebinding/g().


def h(p):
#   ^ definition  snapshot-util 0.1 rebinding/h().
#     ^ definition  snapshot-util 0.1 rebinding/h().(p)
    p = 1
#   ^ reference  snapshot-util 0.1 rebinding/h().(p)
    return p
#          ^ reference  snapshot-util 0.1 rebinding/h().(p)


for x in range(3):
#   ^ reference  snapshot-util 0.1 rebinding/x.
#        ^^^^^ reference  python-stdlib 3.13 builtins/range#
    pass

