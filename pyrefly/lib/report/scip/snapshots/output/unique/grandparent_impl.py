# < definition scip-python python snapshot-util 0.1 grandparent_impl/__init__:

class A:
#     ^ definition  snapshot-util 0.1 grandparent_impl/A#
    def grandparent(self) -> bool:
#       ^^^^^^^^^^^ definition  snapshot-util 0.1 grandparent_impl/A#grandparent().
#                   ^^^^ definition  snapshot-util 0.1 grandparent_impl/A#grandparent().(self)
#                            ^^^^ reference  python-stdlib 3.13 builtins/bool#
        return True

class B(A):
#     ^ definition  snapshot-util 0.1 grandparent_impl/B#
#     relationship implementation scip-python python snapshot-util 0.1 grandparent_impl/A#
#       ^ reference  snapshot-util 0.1 grandparent_impl/A#
    ...

class C(B):
#     ^ definition  snapshot-util 0.1 grandparent_impl/C#
#     relationship implementation scip-python python snapshot-util 0.1 grandparent_impl/B#
#       ^ reference  snapshot-util 0.1 grandparent_impl/B#
    def grandparent(self) -> bool:
#       ^^^^^^^^^^^ definition  snapshot-util 0.1 grandparent_impl/C#grandparent().
#       relationship implementation scip-python python snapshot-util 0.1 grandparent_impl/A#grandparent().
#                   ^^^^ definition  snapshot-util 0.1 grandparent_impl/C#grandparent().(self)
#                            ^^^^ reference  python-stdlib 3.13 builtins/bool#
        return False

