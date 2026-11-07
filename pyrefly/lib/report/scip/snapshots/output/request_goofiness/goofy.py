# < definition scip-python python snapshot-util 0.1 goofy/__init__:

import requests
#      ^^^^^^^^ reference  requests 2.0.0 requests/__init__:

print(requests.get("https://sourcegraph.com"))
#^^^^ reference  python-stdlib 3.13 builtins/print().
#     ^^^^^^^^ reference  requests 2.0.0 requests/__init__:
#              ^^^ reference  requests 2.0.0 `requests.api`/get().

