# BUG: The type parameter has references but no definition occurrence.
# The declaration of `T` and both annotations should share a defined symbol.
def identity[T](x: T) -> T:
    return x
