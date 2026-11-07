class Left:
    def one(self) -> int:
        return 1

    def shared(self) -> bool:
        return False

class Right:
    def two(self):
        return 2

    def shared(self) -> bool:
        return False

class Multi(Left, Right):
    def one(self) -> int:
        return 1

    def two(self):
        return 2

    def three(self):
        return 3

    def shared(self) -> bool:
        return True
