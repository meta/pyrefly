# < definition scip-python python snapshot-util 0.1 builtin_imports/__init__:
#documentation ```python
#            > (module) builtin_imports
#            > ```

# format-options: showDocs

import re
#      ^^ reference  python-stdlib 3.13 re/__init__:
#      external documentation ```python
#                  > (module) re: Module[re]
#                  > ```
from typing import Callable, Optional
#    ^^^^^^ reference  python-stdlib 3.13 typing/__init__:
#    external documentation ```python
#                > (module) typing: Module[typing]
#                > ```
#                  ^^^^^^^^ reference  python-stdlib 3.13 typing/Callable.
#                  external documentation ```python
#                              > (class) Callable: type[Callable]
#                              > ```
#                            ^^^^^^^^ reference  python-stdlib 3.13 typing/Optional.
#                            external documentation ```python
#                                        > (class) Optional: type[Optional]
#                                        > ```

print(re, Callable, Optional)
#^^^^ reference  python-stdlib 3.13 builtins/print().
#external documentation ```python
#            > (function) print: def print(
#            >     *values: object,
#            >     *,
#            >     sep    : str | None                =...
#            >     end    : str | None                =...
#            >     file   : SupportsWrite[str] | None =...
#            >     flush  : Literal[False]            =...
#            > ) -> None: ...
#            > ```
#            > ---
#            > Prints the values to a stream, or to sys...
#            >   
#            > sep  
#            > &nbsp;&nbsp;string inserted between valu...
#            > end  
#            > &nbsp;&nbsp;string appended after the la...
#            > file  
#            > &nbsp;&nbsp;a file-like object (stream);...
#            > flush  
#            > &nbsp;&nbsp;whether to forcibly flush th...
#     ^^ reference  python-stdlib 3.13 re/__init__:
#         ^^^^^^^^ reference  python-stdlib 3.13 typing/Callable.
#                   ^^^^^^^^ reference  python-stdlib 3.13 typing/Optional.

