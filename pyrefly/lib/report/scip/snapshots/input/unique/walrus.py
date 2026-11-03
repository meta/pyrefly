import re

some_list = [1, 2, 3, 4, 5, 100, 1000]
pattern = "^[A-Za-z0-9_]*$"
text = "Some_name123"

# if statement
if (n := len(some_list)) > 10:
    print(f"List is too long with {n} elements.")

if (match := re.search(pattern, text)) is not None:
    print("Found match:", match.group(0))


# comprehensions
def show_some_comprehension():
    [print(x, root) for x in some_list if (x % 2 == 0) and (root := x**0.5) > 5]


# while loop
while (line := input("Enter text: ")) != "quit":
    print(f"You entered: {line}")


# if + comprehension
if any((any_n := num) < 0 for num in some_list):
    print(f"Negative number found: {any_n}")
