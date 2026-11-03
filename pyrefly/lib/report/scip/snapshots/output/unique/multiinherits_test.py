# < definition scip-python python snapshot-util 0.1 multiinherits_test/__init__:

class Left:
#     ^^^^ definition  snapshot-util 0.1 multiinherits_test/Left#
    def one(self) -> int:
#       ^^^ definition  snapshot-util 0.1 multiinherits_test/Left#one().
#           ^^^^ definition  snapshot-util 0.1 multiinherits_test/Left#one().(self)
#                    ^^^ reference  python-stdlib 3.13 builtins/int#
        return 1

    def shared(self) -> bool:
#       ^^^^^^ definition  snapshot-util 0.1 multiinherits_test/Left#shared().
#              ^^^^ definition  snapshot-util 0.1 multiinherits_test/Left#shared().(self)
#                       ^^^^ reference  python-stdlib 3.13 builtins/bool#
        return False

class Right:
#     ^^^^^ definition  snapshot-util 0.1 multiinherits_test/Right#
    def two(self):
#       ^^^ definition  snapshot-util 0.1 multiinherits_test/Right#two().
#           ^^^^ definition  snapshot-util 0.1 multiinherits_test/Right#two().(self)
        return 2

    def shared(self) -> bool:
#       ^^^^^^ definition  snapshot-util 0.1 multiinherits_test/Right#shared().
#              ^^^^ definition  snapshot-util 0.1 multiinherits_test/Right#shared().(self)
#                       ^^^^ reference  python-stdlib 3.13 builtins/bool#
        return False

class Multi(Left, Right):
#     ^^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#
#     relationship implementation scip-python python snapshot-util 0.1 multiinherits_test/Left#
#     relationship implementation scip-python python snapshot-util 0.1 multiinherits_test/Right#
#           ^^^^ reference  snapshot-util 0.1 multiinherits_test/Left#
#                 ^^^^^ reference  snapshot-util 0.1 multiinherits_test/Right#
    def one(self) -> int:
#       ^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#one().
#       relationship implementation scip-python python snapshot-util 0.1 multiinherits_test/Left#one().
#           ^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#one().(self)
#                    ^^^ reference  python-stdlib 3.13 builtins/int#
        return 1

    def two(self):
#       ^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#two().
#       relationship implementation scip-python python snapshot-util 0.1 multiinherits_test/Right#two().
#           ^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#two().(self)
        return 2

    def three(self):
#       ^^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#three().
#             ^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#three().(self)
        return 3

    def shared(self) -> bool:
#       ^^^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#shared().
#       relationship implementation scip-python python snapshot-util 0.1 multiinherits_test/Left#shared().
#              ^^^^ definition  snapshot-util 0.1 multiinherits_test/Multi#shared().(self)
#                       ^^^^ reference  python-stdlib 3.13 builtins/bool#
        return True

