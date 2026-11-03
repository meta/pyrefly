class A:
    def grandparent(self) -> bool:
        return True

class B(A):
    ...

class C(B):
    def grandparent(self) -> bool:
        return False
