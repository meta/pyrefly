# < definition scip-python python snapshot-util 0.1 main/__init__:

# BUG: This dependency inside the project root gets the project's package identity.
# Its symbols should belong to review-dependency 2.0.0.
from review_dependency import Widget
#    ^^^^^^^^^^^^^^^^^ reference  snapshot-util 0.1 review_dependency/__init__:
#                             ^^^^^^ reference  snapshot-util 0.1 review_dependency/Widget#

widget = Widget()
#^^^^^ definition  snapshot-util 0.1 main/widget.
#        ^^^^^^ reference  snapshot-util 0.1 review_dependency/Widget#

