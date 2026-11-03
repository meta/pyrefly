# < definition scip-python python snapshot-util 0.1 main/__init__:

# The declaration of `T` and both annotations should share a defined symbol.
def identity[T](x: T) -> T:
#   ^^^^^^^^ definition  snapshot-util 0.1 main/identity().
#            ^ definition  snapshot-util 0.1 main/identity().[T]
#               ^ definition  snapshot-util 0.1 main/identity().(x)
#                  ^ reference  snapshot-util 0.1 main/identity().[T]
#                        ^ reference  snapshot-util 0.1 main/identity().[T]
    return x
#          ^ reference  snapshot-util 0.1 main/identity().(x)

