# < definition scip-python python snapshot-util 0.1 inherits_class/__init__:

class A:
#     ^ definition  snapshot-util 0.1 inherits_class/A#
    def x(self) -> int:
#       ^ definition  snapshot-util 0.1 inherits_class/A#x().
#         ^^^^ definition  snapshot-util 0.1 inherits_class/A#x().(self)
#                  ^^^ reference  python-stdlib 3.13 builtins/int#
        raise NotImplemented
#             ^^^^^^^^^^^^^^ reference  python-stdlib 3.13 builtins/NotImplemented#

    def matched_despite_different_type(self, x: int):
#       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 inherits_class/A#matched_despite_different_type().
#                                      ^^^^ definition  snapshot-util 0.1 inherits_class/A#matched_despite_different_type().(self)
#                                            ^ definition  snapshot-util 0.1 inherits_class/A#matched_despite_different_type().(x)
#                                               ^^^ reference  python-stdlib 3.13 builtins/int#
        pass

class B(A):
#     ^ definition  snapshot-util 0.1 inherits_class/B#
#     relationship implementation scip-python python snapshot-util 0.1 inherits_class/A#
#       ^ reference  snapshot-util 0.1 inherits_class/A#
    def x(self) -> int:
#       ^ definition  snapshot-util 0.1 inherits_class/B#x().
#       relationship implementation scip-python python snapshot-util 0.1 inherits_class/A#x().
#         ^^^^ definition  snapshot-util 0.1 inherits_class/B#x().(self)
#                  ^^^ reference  python-stdlib 3.13 builtins/int#
        return 5

    def matched_despite_different_type(self, x: int, y: int):
#       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 inherits_class/B#matched_despite_different_type().
#       relationship implementation scip-python python snapshot-util 0.1 inherits_class/A#matched_despite_different_type().
#                                      ^^^^ definition  snapshot-util 0.1 inherits_class/B#matched_despite_different_type().(self)
#                                            ^ definition  snapshot-util 0.1 inherits_class/B#matched_despite_different_type().(x)
#                                               ^^^ reference  python-stdlib 3.13 builtins/int#
#                                                    ^ definition  snapshot-util 0.1 inherits_class/B#matched_despite_different_type().(y)
#                                                       ^^^ reference  python-stdlib 3.13 builtins/int#
        pass

    def unrelated(self):
#       ^^^^^^^^^ definition  snapshot-util 0.1 inherits_class/B#unrelated().
#                 ^^^^ definition  snapshot-util 0.1 inherits_class/B#unrelated().(self)
        pass

