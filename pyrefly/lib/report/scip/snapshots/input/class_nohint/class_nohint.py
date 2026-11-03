# format-options: showDocs

class Example:
    # Note, only y has a type hint
    y: int

    def __init__(self, in_val):
        self.x = in_val
        self.x = self.x + 1
        self.y = in_val

    def something(self):
        print(self.x)
        print(self.y)
