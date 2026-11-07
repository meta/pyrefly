# < definition scip-python python snapshot-util 0.1 main/__init__:

# Its symbols should belong to review-dependency 2.0.0.
from review_dependency import Widget
#    ^^^^^^^^^^^^^^^^^ reference  review-dependency 2.0.0 review_dependency/__init__:
#                             ^^^^^^ reference  review-dependency 2.0.0 review_dependency/Widget#

widget = Widget()
#^^^^^ definition  snapshot-util 0.1 main/widget.
#        ^^^^^^ reference  review-dependency 2.0.0 review_dependency/Widget#

