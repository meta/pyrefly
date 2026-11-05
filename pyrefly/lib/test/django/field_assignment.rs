/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use crate::django_testcase;
use crate::test::util::TestEnv;
use crate::testcase;

django_testcase!(
    test_field_db_default_assignment_contract,
    r#"
from typing import assert_type
from django.db import models
from django.db.models import F

class Record(models.Model):
    count = models.IntegerField(db_default=0)
    optional_count = models.IntegerField(null=True, db_default=0)

assert_type(Record().optional_count, int | None)

def update(record: Record) -> None:
    record.count = 1
    record.count = F("count") + 1
    record.optional_count = None
    record.count = object()  # E: is not assignable to attribute `count`
"#,
);

django_testcase!(
    test_field_intersection_gradual_setter,
    r#"
from typing import Any
from django.db import models

class DynamicField(models.Field):
    _pyi_private_get_type: Any
    _pyi_private_set_type: Any

class AIntRecord:
    value: int

class ZDynamicRecord(models.Model):
    value = DynamicField()

class ADynamicRecord(models.Model):
    value = DynamicField()

class ZIntRecord:
    value: int

def int_first(record: AIntRecord) -> None:
    if isinstance(record, ZDynamicRecord):
        record.value = "bad"  # E: is not assignable to attribute `value`
        record.value = 1

def gradual_first(record: ADynamicRecord) -> None:
    if isinstance(record, ZIntRecord):
        record.value = "bad"  # E: is not assignable to attribute `value`
        record.value = 1
"#,
);

// A gradual setter is a declared contract, not evidence that every value fits the read type.
django_testcase!(
    test_field_gradual_setter_preserves_read_type,
    r#"
from datetime import timedelta
from typing import Any, assert_type
from django.db import models
from django.db.models import F

class DurationField(models.Field):
    _pyi_private_get_type: timedelta
    _pyi_private_set_type: Any

class BinaryField(models.Field):
    _pyi_private_get_type: bytes | memoryview
    _pyi_private_set_type: Any

class Record(models.Model):
    duration = DurationField()
    data = BinaryField()

    def update(self) -> None:
        self.duration = "nope"
        assert_type(self.duration, timedelta)
        self.data = 123
        assert_type(self.data, bytes | memoryview)
        self.duration = F("duration") + timedelta(days=1)
        assert_type(self.duration, timedelta)

def update(record: Record, value: timedelta, data: bytes) -> None:
    record.duration = value
    assert_type(record.duration, timedelta)
    record.data = data
    assert_type(record.data, bytes)
"#,
);

fn incomplete_django_env() -> TestEnv {
    let mut env = TestEnv::new();
    env.add("django", "");
    env.add("django.db", "import django.db.models as models");
    env.add("django.db.models.base", "class Model: ...");
    env.add(
        "django.db.models.fields",
        r#"
class Field:
    def __init__(self, *, null: bool = False, choices: object = None) -> None: ...

class IntegerField(Field):
    _pyi_private_get_type: int

class CharField(Field):
    _pyi_private_get_type: str
    _pyi_private_set_type: str
"#,
    );
    env.add(
        "django.db.models",
        "from django.db.models.base import Model\nfrom django.db.models.fields import IntegerField, CharField",
    );
    env.add("django.db.models.expressions", "");
    env
}

testcase!(
    test_field_missing_setter_marker,
    incomplete_django_env(),
    r#"
from typing import assert_type
from django.db import models

class Record(models.Model):
    count = models.IntegerField()
    optional_count = models.IntegerField(null=True)

assert_type(Record.count, models.IntegerField)

def update(record: Record) -> None:
    assert_type(record.count, int)
    assert_type(record.optional_count, int | None)
    record.count = 1
    record.count = "bad"  # E: is not assignable to attribute `count`
"#,
);

testcase!(
    test_field_choices_missing_expression_export,
    incomplete_django_env(),
    r#"
from typing import Literal, assert_type
from django.db import models

class Record(models.Model):
    choice = models.CharField(choices=(("A", "A"), ("B", "B")))

def update(record: Record) -> None:
    assert_type(record.choice, Literal["A", "B"])
    record.choice = "A"
    record.choice = "bad"  # E: is not assignable to attribute `choice`
"#,
);
