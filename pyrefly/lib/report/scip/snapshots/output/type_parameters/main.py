# < definition scip-python python snapshot-util 0.1 main/__init__:

# BUG: The type parameter has references but no definition occurrence.
# The declaration of `T` and both annotations should share a defined symbol.
def identity[T](x: T) -> T:
#   ^^^^^^^^ definition  snapshot-util 0.1 main/identity().
#               ^ definition  snapshot-util 0.1 main/identity().(x)
#                  ^ reference local 0
#                        ^ reference local 0
    return x
#          ^ reference  snapshot-util 0.1 main/identity().(x)

