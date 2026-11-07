# < definition scip-python python snapshot-util 0.1 comp/__init__:

def something(x):
#   ^^^^^^^^^ definition  snapshot-util 0.1 comp/something().
#             ^ definition  snapshot-util 0.1 comp/something().(x)
    return 2 * x
#              ^ reference  snapshot-util 0.1 comp/something().(x)

_ = [x for x in "should be local 0"]
# definition  snapshot-util 0.1 comp/_.
#    ^ reference local 0
#          ^ definition local 0
_ = [something(x) for x in "should be local 1"]
# reference  snapshot-util 0.1 comp/_.
#    ^^^^^^^^^ reference  snapshot-util 0.1 comp/something().
#              ^ reference local 1
#                     ^ definition local 1
_ = [something(x) for x in "should be local 2" if x == "s"]
# reference  snapshot-util 0.1 comp/_.
#    ^^^^^^^^^ reference  snapshot-util 0.1 comp/something().
#              ^ reference local 2
#                     ^ definition local 2
#                                                 ^ reference local 2
_ = {k: x for (k, x) in enumerate(range(10))}
# reference  snapshot-util 0.1 comp/_.
#    ^ reference local 3
#       ^ reference local 4
#              ^ definition local 3
#                 ^ definition local 4
#                       ^^^^^^^^^ reference  python-stdlib 3.13 builtins/enumerate#
#                                 ^^^^^ reference  python-stdlib 3.13 builtins/range#


asdf = (var for var in [1, 2, 3] if var % 2 == 0)
#^^^ definition  snapshot-util 0.1 comp/asdf.
#       ^^^ reference local 5
#               ^^^ definition local 5
#                                   ^^^ reference local 5
for var in asdf:
#   ^^^ definition  snapshot-util 0.1 comp/var.
#          ^^^^ reference  snapshot-util 0.1 comp/asdf.
    print(var)
#   ^^^^^ reference  python-stdlib 3.13 builtins/print().
#         ^^^ reference  snapshot-util 0.1 comp/var.

# TODO: ListComprehensions with if
# TODO: Dictionary comprehensions


