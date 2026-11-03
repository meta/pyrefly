# < definition scip-python python snapshot-util 0.1 field_docstring/__init__:
#documentation ```python
#            > (module) field_docstring
#            > ```

# format-options: showDocs

class ClassWithField:
#     ^^^^^^^^^^^^^^ definition  snapshot-util 0.1 field_docstring/ClassWithField#
#     documentation ```python
#                 > (class) ClassWithField: type[ClassWithFi...
#                 > ```

    a: int
#   ^ definition  snapshot-util 0.1 field_docstring/ClassWithField#a.
#   documentation ```python
#               > (variable) a: int
#               > ```
#               > ---
#               > Hello world, this is a
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
    """Hello world, this is a"""

