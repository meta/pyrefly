# BUG: The nonlocal assignment has a different symbol from the outer binding.
# All occurrences of `x` should share the same symbol, skipping `middle`.
def outer():
    x = 0

    def middle():
        def inner():
            nonlocal x
            x = 1
            return x

        return inner

    return x, middle
