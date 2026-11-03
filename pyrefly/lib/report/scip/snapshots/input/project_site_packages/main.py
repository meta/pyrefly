# BUG: This dependency inside the project root gets the project's package identity.
# Its symbols should belong to review-dependency 2.0.0.
from review_dependency import Widget

widget = Widget()
