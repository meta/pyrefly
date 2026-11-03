# < definition scip-python python snapshot-util 0.1 main/__init__:

# BUG: The nonlocal assignment has a different symbol from the outer binding.
# All occurrences of `x` should share the same symbol, skipping `middle`.
def outer():
#   ^^^^^ definition  snapshot-util 0.1 main/outer().
    x = 0
#   ^ definition local 0

    def middle():
#       ^^^^^^ definition  snapshot-util 0.1 main/outer().middle().
        def inner():
#           ^^^^^ definition  snapshot-util 0.1 main/outer().middle().inner().
            nonlocal x
            x = 1
#           ^ definition local 1
            return x
#                  ^ reference local 1

        return inner
#              ^^^^^ reference  snapshot-util 0.1 main/outer().middle().inner().

    return x, middle
#          ^ reference local 0
#             ^^^^^^ reference  snapshot-util 0.1 main/outer().middle().

