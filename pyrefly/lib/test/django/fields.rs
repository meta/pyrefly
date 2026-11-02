/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::django_testcase;

django_testcase!(
    test_textfield_nullable,
    r#"
from django.db import models

class Group(models.Model):
    name = models.TextField()

class Customer(models.Model):
    name = models.TextField(null=True)

def test():
    c = Customer.objects.create()
    c.name = None
"#,
);

django_testcase!(
    test_get_foo_display,
    r#"
from typing import assert_type

from django.db import models

class Person(models.Model):
    SHIRT_SIZES = {
        "S": "Small",
        "M": "Medium",
        "L": "Large",
    }
    name = models.CharField(max_length=60)
    shirt_size = models.CharField(max_length=2, choices=SHIRT_SIZES)

p = Person(name="Fred Flintstone", shirt_size="L")
p.save()
assert_type(p.shirt_size, str) 
assert_type(p.get_shirt_size_display(), str)
"#,
);

django_testcase!(
    test_charfield_choices_inline_tuple,
    r#"
from typing import assert_type, Literal

from django.db import models

class Card(models.Model):
    suit = models.CharField(
        max_length=100,
        choices=(
            ("CLUBS", "Clubs"),
            ("SPADES", "Spades"),
            ("HEARTS", "Hearts"),
            ("DIAMONDS", "Diamonds"),
        ),
    )

card = Card(suit="CLUBS")
assert_type(card.suit, Literal["CLUBS", "SPADES", "HEARTS", "DIAMONDS"])
"#,
);

// We can consider narrowing the type further
// but it's unclear if it's worth doing so since
// django stubs give type Any for the aggregate values
django_testcase!(
    test_int_field,
    r#"
from django.db import models
from typing import assert_type, Any

class Default(models.Model):
    int_field = models.IntegerField(default=0)

total_sum_typed = Default.objects.aggregate(
        total=models.Sum("int_field", output_field=models.IntegerField())
)
assert_type(total_sum_typed, dict[str, Any]) 
"#,
);

// Mapping a union of Django field types to their Python types rebuilds the union through the
// solver, so a literal is absorbed by the class the field maps to.
django_testcase!(
    test_field_union_absorbs_literal,
    r#"
from typing import reveal_type
from django.db import models

def flag() -> bool: ...

class M(models.Model):
    x = models.CharField(max_length=10) if flag() else "A"

def probe(m: M) -> None:
    reveal_type(m.x)  # E: revealed type: str
"#,
);

django_testcase!(
    test_queryset_as_manager_preserves_custom_methods,
    r#"
from django.db import models

class NotificationQuerySet(models.QuerySet["Notification"]):
    def resolved(self):
        return self.filter(resolved_at__isnull=False)

class Notification(models.Model):
    resolved_at = models.DateTimeField(null=True, blank=True)
    objects = NotificationQuerySet.as_manager()

Notification.objects.resolved()
Notification.objects.all().resolved()
"#,
);

django_testcase!(
    test_queryset_manager_type,
    r#"
from typing import assert_type
from django.db import models

class Q(models.QuerySet["M"]):
    def active(self) -> int:
        return 1

    def _private(self) -> int:
        return 1

    def excluded(self) -> int:
        return 1
    excluded.queryset_only = True  # type: ignore[missing-attribute]

    def _included(self) -> int:
        return 1
    _included.queryset_only = False  # type: ignore[missing-attribute]

    @property
    def label(self) -> str:
        return "label"

    @classmethod
    def class_method(cls) -> int:
        return 1

class M(models.Model):
    objects = Q.as_manager()

def accept_manager(manager: models.Manager[M]) -> None: ...

accept_manager(M.objects)
assert_type(M.objects, models.Manager[M])
accept_manager(M.objects.db_manager())
assert_type(M.objects.db_manager().active(), int)
assert_type(M.objects.get(), M)
assert_type(M.objects.create(), M)
assert_type(M.objects.get_queryset(), Q)
assert_type(M.objects.all(), Q)
assert_type(M.objects.filter().active(), int)
assert_type(M.objects.active(), int)
assert_type(M.objects._included(), int)
M.objects.delete()  # E: has no attribute `delete`
M.objects._private()  # E: has no attribute `_private`
M.objects.excluded()  # E: has no attribute `excluded`
M.objects.label  # E: has no attribute `label`
M.objects.class_method()  # E: has no attribute `class_method`
M.objects.all().delete()
iter(M.objects)  # E: No matching overload found
"#,
);

django_testcase!(
    test_queryset_manager_model_binding,
    r#"
from typing import Generic, TypeVar, assert_type
from django.db import models

T = TypeVar("T", bound=models.Model)

class Q(models.QuerySet[T], Generic[T]):
    def active(self) -> int:
        return 1

class BareQ(models.QuerySet):
    def active(self) -> int:
        return 1

class Base(models.Model):
    objects = Q.as_manager()
    records = BareQ.as_manager()

    class Meta:
        abstract = True

class Child(Base): ...

assert_type(Base.objects.get(), Base)
assert_type(Child.objects.get(), Child)
assert_type(Child.records.get(), Child)
assert_type(Child.objects.active(), int)
assert_type(Child.objects.all().get(), Child)
assert_type(Child.objects.get_queryset(), Q[Child])
assert_type(Child.objects.filter().get(), Child)
assert_type(Child.records.all().get(), Child)
assert_type(Child.records.all()[0], Child)

class FixedQ(models.QuerySet["FixedBase"]):
    def active(self) -> int:
        return 1

    def accept_base(self, other: "FixedBase") -> "FixedBase":
        return other

class FixedBase(models.Model):
    objects = FixedQ.as_manager()

    class Meta:
        abstract = True

class FixedChild(FixedBase): ...

assert_type(FixedChild.objects.get(), FixedChild)
assert_type(FixedChild.objects.all().get(), FixedChild)
assert_type(FixedChild.objects.all()[0], FixedChild)
for child in FixedChild.objects.all():
    assert_type(child, FixedChild)
assert_type(FixedChild.objects.accept_base(FixedBase()), FixedBase)

class Plain(models.Model):
    objects = models.QuerySet.as_manager()

assert_type(Plain.objects.get(), Plain)
assert_type(Plain.objects.all().get(), Plain)

R = TypeVar("R")

class RowQ(models.QuerySet[T, R], Generic[T, R]):
    def active(self) -> int:
        return 1

class RowModel(models.Model):
    objects = RowQ.as_manager()

assert_type(RowModel.objects.get(), RowModel)
assert_type(RowModel.objects.all().get(), RowModel)
"#,
);

django_testcase!(
    test_queryset_manager_overrides,
    r#"
from django.db import models

class Q1(models.QuerySet):
    def first_method(self) -> int:
        return 1

class Q2(models.QuerySet):
    def second_method(self) -> int:
        return 2

class Base(models.Model):
    records = Q1.as_manager()

class Child(Base):
    records = Q2.as_manager()

Child.records.second_method()

class Parent(models.Model):
    objects: int = 1  # E: overrides ClassVar of the same name in parent class `Model`

class Invalid(Parent):
    objects: str = "bad"  # E: overrides parent class `Parent` in an inconsistent manner

class Pretender:
    @classmethod
    def as_manager(cls) -> str:
        return "not a manager"

class Ordinary(models.Model):
    records = Pretender.as_manager()

Ordinary.records.upper()
"#,
);

django_testcase!(
    test_queryset_manager_composition,
    r#"
from collections.abc import Iterable
from typing import Any, Protocol, Self, assert_type
from django.db import models

class Other(models.Model): ...

class Q(models.QuerySet["M"]):
    def all(self, *, active: bool = False) -> Self:
        return self.filter(active=active)

    def others(self) -> models.QuerySet[Other]:
        return Other.objects.all()

    def names(self) -> models.QuerySet[Other, str]:
        return models.QuerySet[Other, str]()

class M(models.Model):
    objects = Q.as_manager()

assert_type(M.objects.all()[0], M)
for row in M.objects.all():
    assert_type(row, M)
assert_type(M.objects.all().values_list("id", flat=True).first(), Any | None)
assert_type(M.objects.annotate(extra=models.Value(1)).get().extra, Any)
assert_type(M.objects.annotate(extra=models.Value(1)).filter()[0].extra, Any)
assert_type(M.objects.others().get(), Other)
assert_type(M.objects.others().create(), Other)
assert_type(M.objects.others()[0], Other)
assert_type(M.objects.names().get(), str)
assert_type(M.objects.names().first(), str | None)
assert_type(M.objects.names().create(), Other)
M.objects.all(active=True)  # E: Unexpected keyword argument
M.objects.get_queryset().all(active=True)

def preserve[T](value: T) -> T:
    return value

assert_type(preserve(M.objects.all())[0], M)
def accept_iterable(rows: Iterable[M]) -> None: ...
accept_iterable(M.objects.all())

class HasOthers(Protocol):
    def others(self) -> models.QuerySet[Other]: ...

def accept_protocol(manager: HasOthers) -> None: ...
accept_protocol(M.objects)

def choose() -> bool: ...
rows = M.objects.all() if choose() else M.objects.filter()
assert_type(rows[0], M)
"#,
);

django_testcase!(
    test_queryset_manager_specialized_rows,
    r#"
from typing import Generic, TypeVar, assert_type
from django.db import models

T = TypeVar("T", bound=models.Model)

class Q(models.QuerySet[T, tuple[T, int]], Generic[T]): ...

class Base(models.Model):
    records = Q.as_manager()
    objects = models.QuerySet.as_manager()

    class Meta:
        abstract = True

class Child(Base): ...

assert_type(Base.records.get(), tuple[Base, int])
assert_type(Child.records.get(), tuple[Child, int])
assert_type(Child.records.all()[0], tuple[Child, int])
for row in Child.records.all():
    assert_type(row, tuple[Child, int])

def accept_rows(rows: models.QuerySet[Child, tuple[Child, int]]) -> None: ...
accept_rows(Child.records.all())
assert_type(Child.objects.all(), models.QuerySet[Child])
def accept_children(rows: models.QuerySet[Child]) -> None: ...
accept_children(Child.objects.all())

class FixedBase(models.Model): ...

class FixedChild(FixedBase):
    objects = models.QuerySet[FixedBase].as_manager()

assert_type(FixedChild.objects.all(), models.QuerySet[FixedChild])
"#,
);

django_testcase!(
    test_queryset_manager_classmethod_self,
    r#"
from typing import Self, assert_type
from django.db import models

class Q(models.QuerySet["Base"]): ...

class Base(models.Model):
    objects = Q.as_manager()

    @classmethod
    def direct(cls) -> Self:
        return cls.objects.get()

    @classmethod
    def filtered(cls) -> Self:
        return cls.objects.filter().get()

    @classmethod
    def all_rows(cls) -> Self:
        return cls.objects.all().get()

    @classmethod
    def queryset(cls) -> Self:
        return cls.objects.get_queryset().get()

class Child(Base): ...

assert_type(Child.direct(), Child)
assert_type(Child.filtered(), Child)
assert_type(Child.all_rows(), Child)
assert_type(Child.queryset(), Child)
"#,
);

django_testcase!(
    test_queryset_manager_annotated_interfaces,
    r#"
from typing import ClassVar, assert_type
from django.db import models
from django.db.models.manager import BaseManager

class SpecificManager(models.Manager["Base"]):
    def custom(self) -> int:
        return 1

class Q(models.QuerySet["Base"]):
    def active(self) -> int:
        return 1

class Base(models.Model):
    records: SpecificManager = SpecificManager()
    objects: ClassVar[models.Manager["Base"]] = models.Manager()
    entries: BaseManager["Base"] = models.Manager()

    class Meta:
        abstract = True

class Invalid(Base):
    records = Q.as_manager()  # E: not assignable to attribute `records` with type `SpecificManager`

assert_type(Invalid.records.custom(), int)
Invalid.records.active()  # E: has no attribute `active`

class Valid(Base):
    records = SpecificManager()
    objects = Q.as_manager()
    entries = Q.as_manager()

assert_type(Valid.records.custom(), int)
assert_type(Valid.objects.get(), Valid)
assert_type(Valid.objects.active(), int)
assert_type(Valid.entries.get(), Valid)
assert_type(Valid.entries.active(), int)
"#,
);

django_testcase!(
    test_queryset_manager_instance_assignment,
    r#"
from typing import ClassVar, assert_type
from django.db import models

class Q(models.QuerySet["Base"]): ...

class Base(models.Model):
    objects: ClassVar[models.Manager["Base"]] = Q.as_manager()

class Child(Base):
    def reset(self) -> None:
        self.objects = Q.as_manager()  # E: Cannot set field `objects`

class DefaultManager(models.Model):
    def reset(self) -> None:
        self.objects = Q.as_manager()  # E: Cannot set field `objects`

class Ordinary:
    records = Q.as_manager()

assert_type(Ordinary.records.get(), Base)
"#,
);

django_testcase!(
    test_queryset_manager_inferred_custom_interface,
    r#"
from typing import assert_type
from django.db import models
from django.db.models.manager import BaseManager

class ModelBase(models.Model): ...

class SpecificManager(models.Manager["Base"]):
    def custom(self) -> int:
        return 1

class Q(models.QuerySet["Base"]): ...

class Base(ModelBase):
    records = SpecificManager()
    standard = models.Manager[ModelBase]()
    base_manager = BaseManager[ModelBase]()

class InvalidManager(Base):
    records = models.Manager()  # E: overrides parent class `Base` in an inconsistent manner

class InvalidQuerySet(Base):
    records = Q.as_manager()  # E: overrides parent class `Base` in an inconsistent manner

class Valid(Base):
    records = SpecificManager()
    standard = Q.as_manager()
    base_manager = Q.as_manager()

assert_type(Valid.records.custom(), int)
assert_type(Valid.standard.get(), Valid)
assert_type(Valid.base_manager.get(), Valid)
"#,
);

django_testcase!(
    test_queryset_manager_inherited_qualifiers,
    r#"
from typing import ClassVar, Final, assert_type
from django.db import models

class Q(models.QuerySet["Base"]): ...

class Base(models.Model):
    objects: ClassVar[models.Manager["Base"]] = models.Manager()
    records: Final[models.Manager["Base"]] = models.Manager()

class Child(Base):
    objects = Q.as_manager()
    records = Q.as_manager()  # E: `records` is declared as final in parent class `Base`

assert_type(Child.objects.get(), Child)
assert_type(Child.records.get(), Child)
child = Child()
child.objects = Child.objects  # E: Cannot set field `objects`
Child.records = Child.records  # E: Cannot set field `records`

class GrandChild(Child):
    objects = Q.as_manager()
    records = Q.as_manager()  # E: `records` is declared as final in parent class `Child`

assert_type(GrandChild.objects.get(), GrandChild)
grandchild = GrandChild()
grandchild.objects = GrandChild.objects  # E: Cannot set field `objects`
"#,
);

django_testcase!(
    test_queryset_manager_annotated_inherited_qualifiers,
    r#"
from typing import Annotated, ClassVar, Final, assert_type
from django.db import models

class Q(models.QuerySet["Base"]):
    def active(self) -> int:
        return 1

class Base(models.Model):
    records: Annotated[models.Manager["Base"], "doc"] = models.Manager()
    objects: ClassVar[Annotated[models.Manager["Base"], "doc"]] = models.Manager()
    locked: Final[Annotated[models.Manager["Base"], "doc"]] = models.Manager()

class Child(Base):
    records = Q.as_manager()
    objects = Q.as_manager()
    locked = Q.as_manager()  # E: `locked` is declared as final in parent class `Base`

assert_type(Child.records.get(), Child)
assert_type(Child.records.active(), int)
assert_type(Child.objects.get(), Child)
assert_type(Child.locked.get(), Child)
child = Child()
child.objects = Child.objects  # E: Cannot set field `objects`
Child.locked = Child.locked  # E: Cannot set field `locked`

class GrandChild(Child):
    records = Q.as_manager()
    objects = Q.as_manager()
    locked = Q.as_manager()  # E: `locked` is declared as final in parent class `Child`

assert_type(GrandChild.records.get(), GrandChild)
assert_type(GrandChild.objects.get(), GrandChild)
grandchild = GrandChild()
grandchild.objects = GrandChild.objects  # E: Cannot set field `objects`
"#,
);
