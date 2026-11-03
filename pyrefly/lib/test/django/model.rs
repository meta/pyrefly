/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::django_testcase;
use crate::test::django::util::django_env;
use crate::test::django::util::django_env_with_infer_with_first_use;
use crate::test::util::TestEnv;
use crate::testcase;

fn django_model_utils_env() -> TestEnv {
    let mut env = django_env();
    env.add(
        "model_utils.managers",
        r#"
from typing import TypeVar

from django.db import models

ModelT = TypeVar("ModelT", bound=models.Model, covariant=True)

class InheritanceQuerySet(models.QuerySet[ModelT]): ...

class InheritanceManager(models.Manager[ModelT]):
    def select_subclasses(self) -> InheritanceQuerySet[ModelT]: ...
"#,
    );
    env
}

testcase!(
    test_django_model_utils_custom_manager,
    django_model_utils_env(),
    r#"
from typing import assert_type

from django.db import models
from model_utils.managers import InheritanceManager, InheritanceQuerySet

class Place(models.Model):
    objects = InheritanceManager()

assert_type(Place.objects, InheritanceManager[Place])
assert_type(Place.objects.select_subclasses(), InheritanceQuerySet[Place])
assert_type(Place.objects.get(), Place)
assert_type(Place.objects.create(), Place)
assert_type(Place.objects.filter(), models.QuerySet[Place])

class Child(Place):
    objects = models.Manager()

assert_type(Child.objects, models.Manager[Child])
assert_type(Child.objects.get(), Child)

class SiblingManager(models.Manager):
    def custom(self) -> int:
        return 1

class OtherChild(Place):
    objects = SiblingManager()

assert_type(OtherChild.objects, SiblingManager)
assert_type(OtherChild.objects.get(), OtherChild)
assert_type(OtherChild.objects.custom(), int)
"#,
);

django_testcase!(
    test_bare_custom_manager_model_binding,
    r#"
from typing import assert_type
from django.db import models

class PersonManager(models.Manager):
    def custom(self) -> int:
        return 1

class Person(models.Model):
    objects = PersonManager()

class Child(Person): ...

assert_type(Person.objects, PersonManager)
assert_type(Person.objects.get(), Person)
assert_type(Person.objects.create(), Person)
assert_type(Person.objects.filter(), models.QuerySet[Person])
assert_type(Person.objects.model, type[Person])
assert_type(Person.objects.custom(), int)
assert_type(Person.objects.db_manager().get(), Person)
assert_type(Child.objects.get(), Child)
assert_type(Child.objects.filter(), models.QuerySet[Child])

def accept_manager(manager: models.Manager[Person]) -> None: ...
accept_manager(Person.objects)
"#,
);

django_testcase!(
    test_custom_manager_inherited_qualifiers,
    r#"
from typing import Any, ClassVar, Final, Protocol, assert_type
from django.db import models

class CustomManager(models.Manager): ...

class Base(models.Model):
    records: ClassVar[models.Manager["Base"]] = models.Manager()
    locked: Final[models.Manager["Base"]] = models.Manager()

class Person(Base):
    objects = CustomManager()
    records = CustomManager()
    locked = CustomManager()  # E: `locked` is declared as final in parent class `Base`

class HasManager(Protocol):
    objects: models.Manager[Any]

def accept_model(model: HasManager) -> None: ...

class HasCustomManager(Protocol):
    objects: CustomManager

def accept_custom_model(model: HasCustomManager) -> None: ...
accept_custom_model(Person)

person = Person()
person.objects = Person.objects  # E: Cannot set field `objects`
person.records = Person.records  # E: Cannot set field `records`
Person.locked = Person.locked  # E: Cannot set field `locked`

class Child(Person):
    objects = models.Manager()
    records = models.Manager()

assert_type(Child.objects.get(), Child)
assert_type(Child.records.get(), Child)
accept_model(Child)
child = Child()
child.objects = Child.objects  # E: Cannot set field `objects`
child.records = Child.records  # E: Cannot set field `records`

class Plain(models.Model):
    objects = models.Manager()

accept_model(Plain)
plain = Plain()
plain.objects = Plain.objects  # E: Cannot set field `objects`
"#,
);

django_testcase!(
    test_custom_manager_constructor_arguments,
    r#"
from typing import Generic, TypeVar, assert_type
from django.db import models

T = TypeVar("T")
M = TypeVar("M", bound=models.Model)

class CustomManager(models.Manager[M], Generic[T, M]):
    def __init__(self, value: T) -> None:
        self.value = value

    def custom(self) -> T:
        return self.value

class Person(models.Model):
    objects = CustomManager("label")

assert_type(Person.objects, CustomManager[str, Person])
assert_type(Person.objects.custom(), str)
assert_type(Person.objects.get(), Person)

class Ordinary:
    objects = models.Manager[Person]()

assert_type(Ordinary.objects.get(), Person)
"#,
);

django_testcase!(
    test_annotated_custom_manager_replacements,
    r#"
from typing import ClassVar, TypeVar, assert_type
from django.db import models

M = TypeVar("M", bound=models.Model, covariant=True)

class CustomManager(models.Manager[M]):
    def custom(self) -> int:
        return 1

class SiblingManager(models.Manager):
    def sibling(self) -> int:
        return 1

class Parent(models.Model):
    objects = CustomManager()

class Plain(Parent):
    objects: ClassVar[models.Manager["Plain"]] = models.Manager()

class Sibling(Parent):
    objects: ClassVar[SiblingManager] = SiblingManager()

assert_type(Plain.objects.get(), Plain)
assert_type(Sibling.objects.sibling(), int)
Plain().objects = Plain.objects  # E: Cannot set field `objects`
Sibling().objects = Sibling.objects  # E: Cannot set field `objects`

class DeclaredParent(models.Model):
    objects: ClassVar[CustomManager["DeclaredParent"]] = CustomManager()

class InvalidCustom(DeclaredParent):
    objects: ClassVar[models.Manager["InvalidCustom"]] = models.Manager()  # E: overrides parent class `DeclaredParent` in an inconsistent manner

class Middle(DeclaredParent):
    objects = CustomManager()

class InvalidInherited(Middle):
    objects: ClassVar[models.Manager["InvalidInherited"]] = models.Manager()  # E: overrides parent class `Middle` in an inconsistent manner

assert_type(Middle.objects.custom(), int)

class StandardParent(models.Model):
    objects: ClassVar[models.Manager["StandardParent"]] = models.Manager()

class Other(models.Model): ...

class InvalidModel(StandardParent):
    objects: ClassVar[models.Manager[Other]] = models.Manager()  # E: overrides parent class `StandardParent` in an inconsistent manner
"#,
);

testcase!(
    test_manager_binding_without_first_use,
    django_env_with_infer_with_first_use(false).enable_partially_unknown_type_error(),
    r#"
from typing import Generic, TypeVar, assert_type
from django.db import models

M = TypeVar("M", bound=models.Model)
T = TypeVar("T")

class GenericManager(models.Manager[M]): ...
class ExtraManager(models.Manager[M], Generic[T, M]): ...
class BareManager(models.Manager): ...

class Plain(models.Model):
    objects = models.Manager()

class GenericModel(models.Model):
    objects = GenericManager()

class Bare(models.Model):
    objects = BareManager()

class Partial(models.Model):
    records = ExtraManager()  # E: This expression's type is partially unknown

class Other(models.Model): ...

class Complete(models.Model):
    records = ExtraManager[int, Other]()

assert_type(Plain.objects.get(), Plain)
assert_type(GenericModel.objects, GenericManager[GenericModel])
assert_type(Bare.objects.get(), Bare)
assert_type(Complete.records, ExtraManager[int, Complete])
"#,
);

testcase!(
    test_manager_binding_with_first_use,
    django_env().enable_partially_unknown_type_error(),
    r#"
from typing import TypeVar, assert_type
from django.db import models

M = TypeVar("M", bound=models.Model)
class CustomManager(models.Manager[M]): ...

class Person(models.Model):
    objects = models.Manager()
    records = CustomManager()

assert_type(Person.objects.get(), Person)
assert_type(Person.records, CustomManager[Person])
"#,
);

django_testcase!(
    test_manager_binding_constructor_boundaries,
    r#"
from typing import TypeVar, assert_type
from django.db import models

M = TypeVar("M", bound=models.Model)
class RequiredManager(models.Manager[M]):
    def __init__(self, value: int) -> None:
        self.value = value

class Person(models.Model):
    objects = RequiredManager("bad")  # E: not assignable to parameter `value` with type `int`
    records = RequiredManager()  # E: Missing argument `value`

assert_type(Person.objects.get(), Person)
assert_type(Person.records.get(), Person)

class Other(models.Model): ...
class DifferentManager(models.Manager):
    def __new__(cls) -> models.Manager[Other]:
        return models.Manager[Other]()

def factory() -> models.Manager[Other]:
    return models.Manager[Other]()

class Boundary(models.Model):
    factory_records = factory()
    different_records = DifferentManager()

assert_type(Boundary.factory_records.get(), Other)
assert_type(Boundary.different_records.get(), Other)
"#,
);

django_testcase!(
    test_model,
    r#"
from typing import assert_type

from django.db import models

class Person(models.Model):
    first_name = models.CharField(max_length=30)

p = Person(first_name="Alice")
assert_type(p.first_name, str)
"#,
);

django_testcase!(
    test_model_admin_tuple,
    r#"
from django.contrib import admin
from django.db import models

class Person(models.Model):
    first_name = models.CharField(max_length=None)
    last_name = models.CharField(max_length=None)
    birthday = models.DateField()

class PersonFieldsetTupleAdmin(admin.ModelAdmin[Person]):
    fieldsets = (
        (
            "Personal Details",
            {
                "description": "Personal details of a person.",
                "fields": (("first_name", "last_name"), "birthday"),
            },
        ),
    )
"#,
);

django_testcase!(
    test_model_admin_list,
    r#"
from django.contrib import admin 
from django.db import models

class Person(models.Model):
    first_name = models.CharField(max_length=None)
    last_name = models.CharField(max_length=None)
    birthday = models.DateField()

class PersonFieldsetListAdmin(admin.ModelAdmin[Person]):
    fieldsets = [
        (
            "Personal Details",
            {
                "description": "Personal details of a person.",
                "fields": [["first_name", "last_name"], "birthday"],
            },
        )
    ]
"#,
);

django_testcase!(
    test_meta_override_without_inheritance,
    r#"
from django.db import models

class DateTimeMixin(models.Model):

    created_at = models.DateTimeField(auto_now_add=True, editable=False)
    updated_at = models.DateTimeField(auto_now=True)

    class Meta:
        abstract = True

class Invoice(DateTimeMixin):
    class Meta:
        verbose_name = "Invoice"
        verbose_name_plural = "Invoices"
"#,
);
