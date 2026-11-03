# < definition scip-python python snapshot-util 0.1 main/__init__:

# Each `hidden` should have a distinct local symbol.
first = lambda: (hidden := 1)
#^^^^ definition  snapshot-util 0.1 main/first.
#                ^^^^^^ definition local 0
second = lambda: (hidden := 2)
#^^^^^ definition  snapshot-util 0.1 main/second.
#                 ^^^^^^ definition local 1

