class A:
    def x(self) -> int:
        raise NotImplemented

    def matched_despite_different_type(self, x: int):
        pass

class B(A):
    def x(self) -> int:
        return 5

    def matched_despite_different_type(self, x: int, y: int):
        pass

    def unrelated(self):
        pass
