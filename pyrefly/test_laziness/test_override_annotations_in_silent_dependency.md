# Override annotations in a silent dependency

Checking a caller solves the dependency's method signature, but explicit-any
filtering must not add inherited-signature demands when that module is silent.

## Files

`a.py`:
```python
from b import Child
Child().f(1)
```

`b.py`:
```python
from typing import Any, override
from c import Base

class Child(Base):
    @override
    def f(self, value: Any) -> Any:
        return value
```

`c.py`:
```python
from typing import Any

class Base:
    def f(self, value: Any) -> Any:
        return value
```

## Check `a.py`

```expected
a: Solutions
b: Answers
c: Answers

(67 builtin demands hidden)
a -> b::Exports(is_special_export)
a -> b::Load(module_exists)
a -> b::Exports(export_exists)
a -> b::Exports(is_implicit_reexport)
a -> b::Exports(get_deprecated)
a -> b::KeyExport(Name("Child"))
  b -> c::Exports(is_special_export)
a -> b::KeyClassMetadata(ClassDefIndex(0))
  b -> c::Exports(export_exists)
  b -> c::Exports(is_implicit_reexport)
  b -> c::Exports(get_deprecated)
  b -> c::KeyExport(Name("Base"))
  b -> c::KeyClassMetadata(ClassDefIndex(0))
  b -> c::KeyClassMetadata(ClassDefIndex(0))
  b -> c::KeyClassMetadata(ClassDefIndex(0))
a -> b::KeyClassMetadata(ClassDefIndex(0))
a -> b::KeyAbstractClassCheck(ClassDefIndex(0))
  b -> c::KeyAbstractClassCheck(ClassDefIndex(0))
a -> b::KeyClassSynthesizedFields(ClassDefIndex(0))
a -> b::KeyClassMro(ClassDefIndex(0))
  b -> c::KeyClassMetadata(ClassDefIndex(0))
  b -> c::KeyClassBaseType(ClassDefIndex(0))
  b -> c::KeyClassMro(ClassDefIndex(0))
a -> c::KeyClassSynthesizedFields(ClassDefIndex(0))
a -> b::KeyClassMro(ClassDefIndex(0))
a -> b::KeyClassField(ClassDefIndex(0), Name("f"))
  b -> c::KeyClassField(ClassDefIndex(0), Name("f"))
```
