def something(x):
    return 2 * x

_ = [x for x in "should be local 0"]
_ = [something(x) for x in "should be local 1"]
_ = [something(x) for x in "should be local 2" if x == "s"]
_ = {k: x for (k, x) in enumerate(range(10))}


asdf = (var for var in [1, 2, 3] if var % 2 == 0)
for var in asdf:
    print(var)

# TODO: ListComprehensions with if
# TODO: Dictionary comprehensions

