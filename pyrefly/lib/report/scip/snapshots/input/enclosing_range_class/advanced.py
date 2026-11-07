# format-options: showRanges
def class_decorator(cls):
    def wrapper(*args, **kwargs):
        return cls(*args, **kwargs)
    return wrapper

@class_decorator
class Test:
    def __init__(self, x: float):
        self.x = x

    def test(self) -> float:
        return self.x
