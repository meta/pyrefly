# < definition scip-python python snapshot-util 0.1 advanced/__init__:

# format-options: showRanges
# < start enclosing_range scip-python python snapshot-util 0.1 advanced/decorator().
def decorator(func):
#   ^^^^^^^^^ definition  snapshot-util 0.1 advanced/decorator().
#             ^^^^ definition  snapshot-util 0.1 advanced/decorator().(func)
#   ⌄ start enclosing_range scip-python python snapshot-util 0.1 advanced/decorator().wrapper().
    def wrapper(*args, **kwargs):
#       ^^^^^^^ definition  snapshot-util 0.1 advanced/decorator().wrapper().
#                ^^^^ definition  snapshot-util 0.1 advanced/decorator().wrapper().(args)
#                        ^^^^^^ definition  snapshot-util 0.1 advanced/decorator().wrapper().(kwargs)
        return func(*args, **kwargs)
#              ^^^^ reference  snapshot-util 0.1 advanced/decorator().(func)
#                    ^^^^ reference  snapshot-util 0.1 advanced/decorator().wrapper().(args)
#                            ^^^^^^ reference  snapshot-util 0.1 advanced/decorator().wrapper().(kwargs)
#   ^ end enclosing_range scip-python python snapshot-util 0.1 advanced/decorator().wrapper().
    return wrapper
#          ^^^^^^^ reference  snapshot-util 0.1 advanced/decorator().wrapper().
# < end enclosing_range scip-python python snapshot-util 0.1 advanced/decorator().

# < start enclosing_range scip-python python snapshot-util 0.1 advanced/func().
@decorator
#^^^^^^^^^ reference  snapshot-util 0.1 advanced/decorator().
def func(x: float) -> float:
#   ^^^^ definition  snapshot-util 0.1 advanced/func().
#        ^ definition  snapshot-util 0.1 advanced/func().(x)
#           ^^^^^ reference  python-stdlib 3.13 builtins/float#
#                     ^^^^^ reference  python-stdlib 3.13 builtins/float#
    test = x
#   ^^^^ definition local 0
#          ^ reference  snapshot-util 0.1 advanced/func().(x)

    return test
#          ^^^^ reference local 0
# < end enclosing_range scip-python python snapshot-util 0.1 advanced/func().

