# < definition scip-python python snapshot-util 0.1 walrus/__init__:

import re
#      ^^ reference  python-stdlib 3.13 re/__init__:

some_list = [1, 2, 3, 4, 5, 100, 1000]
#^^^^^^^^ definition  snapshot-util 0.1 walrus/some_list.
pattern = "^[A-Za-z0-9_]*$"
#^^^^^^ definition  snapshot-util 0.1 walrus/pattern.
text = "Some_name123"
#^^^ definition  snapshot-util 0.1 walrus/text.

# if statement
if (n := len(some_list)) > 10:
#   ^ definition  snapshot-util 0.1 walrus/n.
#        ^^^ reference  python-stdlib 3.13 builtins/len().
#            ^^^^^^^^^ reference  snapshot-util 0.1 walrus/some_list.
    print(f"List is too long with {n} elements.")
#   ^^^^^ reference  python-stdlib 3.13 builtins/print().
#                                  ^ reference  snapshot-util 0.1 walrus/n.

if (match := re.search(pattern, text)) is not None:
#   ^^^^^ definition  snapshot-util 0.1 walrus/match.
#            ^^ reference  python-stdlib 3.13 re/__init__:
#               ^^^^^^ reference  python-stdlib 3.13 re/search().
#                      ^^^^^^^ reference  snapshot-util 0.1 walrus/pattern.
#                               ^^^^ reference  snapshot-util 0.1 walrus/text.
    print("Found match:", match.group(0))
#   ^^^^^ reference  python-stdlib 3.13 builtins/print().
#                         ^^^^^ reference  snapshot-util 0.1 walrus/match.
#                               ^^^^^ reference  python-stdlib 3.13 re/Match#group().


# comprehensions
def show_some_comprehension():
#   ^^^^^^^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 walrus/show_some_comprehension().
    [print(x, root) for x in some_list if (x % 2 == 0) and (root := x**0.5) > 5]
#    ^^^^^ reference  python-stdlib 3.13 builtins/print().
#          ^ reference local 0
#             ^^^^ reference local 1
#                       ^ definition local 0
#                            ^^^^^^^^^ reference  snapshot-util 0.1 walrus/some_list.
#                                          ^ reference local 0
#                                                           ^^^^ definition local 1
#                                                                   ^ reference local 0


# while loop
while (line := input("Enter text: ")) != "quit":
#      ^^^^ definition  snapshot-util 0.1 walrus/line.
#              ^^^^^ reference  python-stdlib 3.13 builtins/input().
    print(f"You entered: {line}")
#   ^^^^^ reference  python-stdlib 3.13 builtins/print().
#                         ^^^^ reference  snapshot-util 0.1 walrus/line.


# if + comprehension
if any((any_n := num) < 0 for num in some_list):
#  ^^^ reference  python-stdlib 3.13 builtins/any().
#       ^^^^^ definition  snapshot-util 0.1 walrus/any_n.
#                ^^^ reference local 2
#                             ^^^ definition local 2
#                                    ^^^^^^^^^ reference  snapshot-util 0.1 walrus/some_list.
    print(f"Negative number found: {any_n}")
#   ^^^^^ reference  python-stdlib 3.13 builtins/print().
#                                   ^^^^^ reference  snapshot-util 0.1 walrus/any_n.

