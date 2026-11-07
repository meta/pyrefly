from foo.bar import InitClass
from foo.bar.baz.mod import SuchNestedMuchWow, AnotherNestedMuchWow

print(SuchNestedMuchWow().class_item)
print(AnotherNestedMuchWow().other_item)
print(InitClass().init_item)
