# < definition scip-python python snapshot-util 0.1 class_nohint/__init__:
#documentation ```python
#            > (module) class_nohint
#            > ```

# format-options: showDocs

class Example:
#     ^^^^^^^ definition  snapshot-util 0.1 class_nohint/Example#
#     documentation ```python
#                 > (class) Example: type[Example]
#                 > ```
    # Note, only y has a type hint
    y: int
#   ^ definition  snapshot-util 0.1 class_nohint/Example#y.
#   documentation ```python
#               > (variable) y: int
#               > ```
#      ^^^ reference  python-stdlib 3.13 builtins/int#
#      external documentation ```python
#                  > (class) int: 
#                  > @overload
#                  > def __new__(x: ConvertibleToInt = 0, /) ...
#                  > def __new__(
#                  >     x: bytearray | bytes | str,
#                  >     /,
#                  >     base: SupportsIndex
#                  > ) -> int: ...
#                  > ```
#                  > ---
#                  > int([x]) -> integer  
#                  > int(x, base=10) -> integer  
#                  >   
#                  > Convert a number or string to an integer...
#                  > are given.  If x is a number, return x._...
#                  > numbers, this truncates towards zero.  
#                  >   
#                  > If x is not a number or if base is given...
#                  > bytes, or bytearray instance representin...
#                  > given base.  The literal can be preceded...
#                  > by whitespace.  The base defaults to 10....
#                  > Base 0 means to interpret the base from ...
#                  > ```python  
#                  > >>> int('0b100', base=0)  
#                  > ```  
#                  > 4

    def __init__(self, in_val):
#       ^^^^^^^^ definition  snapshot-util 0.1 class_nohint/Example#__init__().
#       documentation ```python
#                   > (method) __init__: def __init__(
#                   >     self: Self@Example,
#                   >     in_val: Unknown
#                   > ) -> None: ...
#                   > ```
#                ^^^^ definition  snapshot-util 0.1 class_nohint/Example#__init__().(self)
#                documentation ```python
#                            > (parameter) self: Self@Example
#                            > ```
#                      ^^^^^^ definition  snapshot-util 0.1 class_nohint/Example#__init__().(in_val)
#                      documentation ```python
#                                  > (parameter) in_val: Unknown
#                                  > ```
        self.x = in_val
#       ^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(self)
#            ^ definition  snapshot-util 0.1 class_nohint/Example#x.
#            documentation ```python
#                        > (attribute) x: Unknown
#                        > ```
#                ^^^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(in_val)
        self.x = self.x + 1
#       ^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(self)
#            ^ reference  snapshot-util 0.1 class_nohint/Example#x.
#                ^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(self)
#                     ^ reference  snapshot-util 0.1 class_nohint/Example#x.
        self.y = in_val
#       ^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(self)
#            ^ reference  snapshot-util 0.1 class_nohint/Example#y.
#                ^^^^^^ reference  snapshot-util 0.1 class_nohint/Example#__init__().(in_val)

    def something(self):
#       ^^^^^^^^^ definition  snapshot-util 0.1 class_nohint/Example#something().
#       documentation ```python
#                   > (method) something: def something(self: ...
#                   > ```
#                 ^^^^ definition  snapshot-util 0.1 class_nohint/Example#something().(self)
#                 documentation ```python
#                             > (parameter) self: Self@Example
#                             > ```
        print(self.x)
#       ^^^^^ reference  python-stdlib 3.13 builtins/print().
#       external documentation ```python
#                   > (function) print: def print(
#                   >     *values: object,
#                   >     *,
#                   >     sep    : str | None                =...
#                   >     end    : str | None                =...
#                   >     file   : SupportsWrite[str] | None =...
#                   >     flush  : Literal[False]            =...
#                   > ) -> None: ...
#                   > ```
#                   > ---
#                   > Prints the values to a stream, or to sys...
#                   >   
#                   > sep  
#                   > &nbsp;&nbsp;string inserted between valu...
#                   > end  
#                   > &nbsp;&nbsp;string appended after the la...
#                   > file  
#                   > &nbsp;&nbsp;a file-like object (stream);...
#                   > flush  
#                   > &nbsp;&nbsp;whether to forcibly flush th...
#             ^^^^ reference  snapshot-util 0.1 class_nohint/Example#something().(self)
#                  ^ reference  snapshot-util 0.1 class_nohint/Example#x.
        print(self.y)
#       ^^^^^ reference  python-stdlib 3.13 builtins/print().
#             ^^^^ reference  snapshot-util 0.1 class_nohint/Example#something().(self)
#                  ^ reference  snapshot-util 0.1 class_nohint/Example#y.

