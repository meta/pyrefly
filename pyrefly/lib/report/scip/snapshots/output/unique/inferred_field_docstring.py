# < definition scip-python python snapshot-util 0.1 inferred_field_docstring/__init__:
#documentation ```python
#            > (module) inferred_field_docstring
#            > ```

# format-options: showDocs

class ClassWithInferredField:
#     ^^^^^^^^^^^^^^^^^^^^^^ definition  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#
#     documentation ```python
#                 > (class) ClassWithInferredField: type[Cla...
#                 > ```
    def __init__(self, b: int):
#       ^^^^^^^^ definition  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#__init__().
#       documentation ```python
#                   > (method) __init__: def __init__(
#                   >     self: Self@ClassWithInferredField,
#                   >     b: int
#                   > ) -> None: ...
#                   > ```
#                ^^^^ definition  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#__init__().(self)
#                documentation ```python
#                            > (parameter) self: Self@ClassWithInferred...
#                            > ```
#                      ^ definition  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#__init__().(b)
#                      documentation ```python
#                                  > (parameter) b: int
#                                  > ```
#                         ^^^ reference  python-stdlib 3.13 builtins/int#
#                         external documentation ```python
#                                     > (class) int: 
#                                     > @overload
#                                     > def __new__(x: ConvertibleToInt = 0, /) ...
#                                     > def __new__(
#                                     >     x: bytearray | bytes | str,
#                                     >     /,
#                                     >     base: SupportsIndex
#                                     > ) -> int: ...
#                                     > ```
#                                     > ---
#                                     > int([x]) -> integer  
#                                     > int(x, base=10) -> integer  
#                                     >   
#                                     > Convert a number or string to an integer...
#                                     > are given.  If x is a number, return x._...
#                                     > numbers, this truncates towards zero.  
#                                     >   
#                                     > If x is not a number or if base is given...
#                                     > bytes, or bytearray instance representin...
#                                     > given base.  The literal can be preceded...
#                                     > by whitespace.  The base defaults to 10....
#                                     > Base 0 means to interpret the base from ...
#                                     > ```python  
#                                     > >>> int('0b100', base=0)  
#                                     > ```  
#                                     > 4
        self.b = b
#       ^^^^ reference  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#__init__().(self)
#            ^ definition  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#b.
#            documentation ```python
#                        > (attribute) b: int
#                        > ```
#                ^ reference  snapshot-util 0.1 inferred_field_docstring/ClassWithInferredField#__init__().(b)

