/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! Classes that the type checker recognizes by their qualified name.

use dupe::Dupe;
use pyrefly_python::module_name::ModuleName;
use pyrefly_util::assert_bytes;

macro_rules! known_classes {
    ($($variant:ident => ($module:literal, $name:literal),)*) => {
        /// A class that the type checker treats specially, identified by its module and the
        /// name it is defined with at the top level of that module.
        ///
        /// Variants are named after the class in PascalCase. Classes outside `builtins` are
        /// prefixed with their library (for a public standard-library submodule such as
        /// `collections.abc`, the submodule), without repeating a class name that starts with
        /// the library name. When two entries would get the same name, the one in the deeper
        /// module adds the module component that tells them apart.
        #[derive(Debug, Clone, Copy, Dupe, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum KnownClass {
            $($variant,)*
        }

        // These tags are meant to stay a small, deliberate set, so growing the table past 256
        // entries needs a design decision rather than a wider representation.
        const _: () = assert!(
            [$(KnownClass::$variant),*].len() <= 256,
            "`KnownClass` is limited to 256 entries"
        );

        impl KnownClass {
            /// The known class defined at the top level of `module` as `name`, if any.
            pub fn lookup(module: ModuleName, name: &str) -> Option<Self> {
                match (module.as_str(), name) {
                    $(($module, $name) => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

// `None` takes a tag value that no entry uses, so this also needs the table to leave one free.
assert_bytes!(Option<KnownClass>, 1);

known_classes! {
    // builtins
    Bool => ("builtins", "bool"),
    ByteArray => ("builtins", "bytearray"),
    Bytes => ("builtins", "bytes"),
    ClassMethod => ("builtins", "classmethod"),
    Complex => ("builtins", "complex"),
    Dict => ("builtins", "dict"),
    Enumerate => ("builtins", "enumerate"),
    Float => ("builtins", "float"),
    FrozenDict => ("builtins", "frozendict"),
    FrozenSet => ("builtins", "frozenset"),
    Int => ("builtins", "int"),
    List => ("builtins", "list"),
    MemoryView => ("builtins", "memoryview"),
    NotImplementedType => ("builtins", "_NotImplementedType"),
    Object => ("builtins", "object"),
    Property => ("builtins", "property"),
    Range => ("builtins", "range"),
    Sentinel => ("builtins", "sentinel"),
    Set => ("builtins", "set"),
    Slice => ("builtins", "slice"),
    StaticMethod => ("builtins", "staticmethod"),
    Str => ("builtins", "str"),
    Super => ("builtins", "super"),
    Tuple => ("builtins", "tuple"),
    Type => ("builtins", "type"),
    Zip => ("builtins", "zip"),

    // Standard library
    TypeshedNamedTupleFallback => ("_typeshed._type_checker_internals", "NamedTupleFallback"),
    TypeshedTypedDictFallback => ("_typeshed._type_checker_internals", "TypedDictFallback"),
    Abc => ("abc", "ABC"),
    AbcMeta => ("abc", "ABCMeta"),
    AbcAbstractClassMethod => ("abc", "abstractclassmethod"),
    AbcAbstractProperty => ("abc", "abstractproperty"),
    AbcAbstractStaticMethod => ("abc", "abstractstaticmethod"),
    ArgparseNamespace => ("argparse", "Namespace"),
    CollectionsUserString => ("collections", "UserString"),
    CollectionsDeque => ("collections", "deque"),
    CollectionsAbcAsyncIterator => ("collections.abc", "AsyncIterator"),
    CollectionsAbcIterator => ("collections.abc", "Iterator"),
    DataclassesField => ("dataclasses", "Field"),
    DataclassesInitVar => ("dataclasses", "InitVar"),
    DataclassesKwOnly => ("dataclasses", "KW_ONLY"),
    DatetimeDate => ("datetime", "date"),
    Datetime => ("datetime", "datetime"),
    DatetimeTime => ("datetime", "time"),
    DatetimeTimedelta => ("datetime", "timedelta"),
    Decimal => ("decimal", "Decimal"),
    Enum => ("enum", "Enum"),
    EnumIntEnum => ("enum", "IntEnum"),
    EnumStrEnum => ("enum", "StrEnum"),
    EnumAuto => ("enum", "auto"),
    EnumMember => ("enum", "member"),
    EnumNonmember => ("enum", "nonmember"),
    EnumProperty => ("enum", "property"),
    FractionsFraction => ("fractions", "Fraction"),
    FunctoolsSingleDispatchCallable => ("functools", "_SingleDispatchCallable"),
    FunctoolsWrapped => ("functools", "_Wrapped"),
    FunctoolsLruCacheWrapper => ("functools", "_lru_cache_wrapper"),
    FunctoolsCachedProperty => ("functools", "cached_property"),
    FunctoolsPartial => ("functools", "partial"),
    RePattern => ("re", "Pattern"),
    ReRegexFlag => ("re", "RegexFlag"),
    TypesDynamicClassAttribute => ("types", "DynamicClassAttribute"),
    TypesEllipsisType => ("types", "EllipsisType"),
    TypesFunctionType => ("types", "FunctionType"),
    TypesGenericAlias => ("types", "GenericAlias"),
    TypesMethodType => ("types", "MethodType"),
    TypesModuleType => ("types", "ModuleType"),
    TypesNoneType => ("types", "NoneType"),
    TypesNotImplementedType => ("types", "NotImplementedType"),
    TypesUnionType => ("types", "UnionType"),
    TypingAny => ("typing", "Any"),
    TypingAsyncIterator => ("typing", "AsyncIterator"),
    TypingCollection => ("typing", "Collection"),
    TypingContainer => ("typing", "Container"),
    TypingIterable => ("typing", "Iterable"),
    TypingIterator => ("typing", "Iterator"),
    TypingNamedTuple => ("typing", "NamedTuple"),
    TypingPattern => ("typing", "Pattern"),
    TypingSequence => ("typing", "Sequence"),
    UnittestTestCase => ("unittest.case", "TestCase"),
    UnittestMockPatcher => ("unittest.mock", "_patcher"),

    // Third-party libraries
    StaticCbool => ("__static__", "cbool"),
    StaticChar => ("__static__", "char"),
    StaticDouble => ("__static__", "double"),
    StaticInt8 => ("__static__", "int8"),
    StaticInt16 => ("__static__", "int16"),
    StaticInt32 => ("__static__", "int32"),
    StaticInt64 => ("__static__", "int64"),
    StaticSingle => ("__static__", "single"),
    StaticUint8 => ("__static__", "uint8"),
    StaticUint16 => ("__static__", "uint16"),
    StaticUint32 => ("__static__", "uint32"),
    StaticUint64 => ("__static__", "uint64"),
    AttrNothing => ("attr", "_Nothing"),
    AttrsNothing => ("attrs", "_Nothing"),
    CachedProperty => ("cached_property", "cached_property"),
    CachedPropertyThreadedCachedProperty => ("cached_property", "threaded_cached_property"),
    CinderAsyncCachedProperty => ("cinder", "async_cached_property"),
    CinderCachedProperty => ("cinder", "cached_property"),
    DjangoModel => ("django.db.models.base", "Model"),
    DjangoChoices => ("django.db.models.enums", "Choices"),
    DjangoAutoField => ("django.db.models.fields", "AutoField"),
    DjangoCharField => ("django.db.models.fields", "CharField"),
    DjangoField => ("django.db.models.fields", "Field"),
    DjangoForeignKey => ("django.db.models.fields.related", "ForeignKey"),
    DjangoManyToManyField => ("django.db.models.fields.related", "ManyToManyField"),
    DjangoOneToOneField => ("django.db.models.fields.related", "OneToOneField"),
    DjangoManyRelatedManager => ("django.db.models.fields.related_descriptors", "ManyRelatedManager"),
    DjangoRelatedManager => ("django.db.models.fields.related_descriptors", "RelatedManager"),
    DjangoBaseManager => ("django.db.models.manager", "BaseManager"),
    DjangoManager => ("django.db.models.manager", "Manager"),
    DjangoQuerySet => ("django.db.models.query", "QuerySet"),
    DjangoGetter => ("django.utils.functional", "_Getter"),
    Factory => ("factory.base", "Factory"),
    FastApi => ("fastapi", "FastAPI"),
    FastApiApplications => ("fastapi.applications", "FastAPI"),
    FastApiTestClient => ("fastapi.testclient", "TestClient"),
    LoggingLogger => ("logging", "Logger"),
    MarshmallowSchema => ("marshmallow.schema", "Schema"),
    NarwhalsDataFrame => ("narwhals.dataframe", "DataFrame"),
    NarwhalsLazyFrame => ("narwhals.dataframe", "LazyFrame"),
    NarwhalsExpr => ("narwhals.expr", "Expr"),
    NarwhalsSeries => ("narwhals.series", "Series"),
    NarwhalsStableV1DataFrame => ("narwhals.stable.v1", "DataFrame"),
    NarwhalsStableV1Expr => ("narwhals.stable.v1", "Expr"),
    NarwhalsStableV1LazyFrame => ("narwhals.stable.v1", "LazyFrame"),
    NarwhalsStableV1Series => ("narwhals.stable.v1", "Series"),
    NarwhalsStableV2DataFrame => ("narwhals.stable.v2", "DataFrame"),
    NarwhalsStableV2Expr => ("narwhals.stable.v2", "Expr"),
    NarwhalsStableV2LazyFrame => ("narwhals.stable.v2", "LazyFrame"),
    NarwhalsStableV2Series => ("narwhals.stable.v2", "Series"),
    PandasSequenceNotStr => ("pandas._typing", "SequenceNotStr"),
    PandasDataFrame => ("pandas.core.frame", "DataFrame"),
    PolarsDataFrame => ("polars.dataframe.frame", "DataFrame"),
    PolarsDataFrameSeries => ("polars.dataframe.frame", "Series"),
    PolarsExpr => ("polars.expr.expr", "Expr"),
    PolarsCol => ("polars.functions.col", "Col"),
    PolarsLazyFrame => ("polars.lazyframe.frame", "LazyFrame"),
    PolarsSchema => ("polars.schema", "Schema"),
    PolarsSeries => ("polars.series.series", "Series"),
    PydanticBaseModel => ("pydantic.main", "BaseModel"),
    PydanticRootModel => ("pydantic.root_model", "RootModel"),
    PydanticStrict => ("pydantic.types", "Strict"),
    PydanticSettingsBaseSettings => ("pydantic_settings.main", "BaseSettings"),
    PyreExtensionsPyreReadOnly => ("pyre_extensions", "PyreReadOnly"),
    PyreExtensionsReadOnly => ("pyre_extensions", "ReadOnly"),
    RestFrameworkField => ("rest_framework.fields", "Field"),
    RestFrameworkModelSerializer => ("rest_framework.serializers", "ModelSerializer"),
    RestFrameworkSerializer => ("rest_framework.serializers", "Serializer"),
    ShapeExtensionsCaptureNamedInts => ("shape_extensions", "CaptureNamedInts"),
    ShapeExtensionsElements => ("shape_extensions", "Elements"),
    ShapeExtensionsFlag => ("shape_extensions", "Flag"),
    ShapeExtensionsIndex => ("shape_extensions", "Index"),
    ShapeExtensionsInt => ("shape_extensions", "Int"),
    ShapeExtensionsIntListLiteral => ("shape_extensions", "IntListLiteral"),
    ShapeExtensionsIntTuple => ("shape_extensions", "IntTuple"),
    ShapeExtensionsIntTuples => ("shape_extensions", "IntTuples"),
    ShapeExtensionsIntVar => ("shape_extensions", "IntVar"),
    ShapeExtensionsMapIntTuples => ("shape_extensions", "MapIntTuples"),
    ShapeExtensionsNamedInts => ("shape_extensions", "NamedInts"),
    ShapeExtensionsProxyMethod => ("shape_extensions", "ProxyMethod"),
    ShapeExtensionsRegularNestedList => ("shape_extensions", "RegularNestedList"),
    ShapeExtensionsDslInt => ("shape_extensions.dsl", "Int"),
    ShapeExtensionsDslIntTuple => ("shape_extensions.dsl", "IntTuple"),
    ShapeExtensionsDslIntTuples => ("shape_extensions.dsl", "IntTuples"),
    SingleDispatchCallable => ("singledispatch", "_SingleDispatchCallable"),
    SqlAlchemyMapped => ("sqlalchemy.orm", "Mapped"),
    SqlAlchemyBaseMapped => ("sqlalchemy.orm.base", "Mapped"),
    SqlAlchemyMappedColumn => ("sqlalchemy.orm.properties", "MappedColumn"),
    SqlAlchemyEnum => ("sqlalchemy.sql.sqltypes", "Enum"),
    SqlAlchemyTypeEngine => ("sqlalchemy.sql.type_api", "TypeEngine"),
    SqlModel => ("sqlmodel", "SQLModel"),
    SqlModelMain => ("sqlmodel.main", "SQLModel"),
    Starlette => ("starlette.applications", "Starlette"),
    StarletteTestClient => ("starlette.testclient", "TestClient"),
    TorchTensor => ("torch", "Tensor"),
    TorchTensorTensor => ("torch._tensor", "Tensor"),
    TorchModule => ("torch.nn", "Module"),
    TorchModuleDict => ("torch.nn", "ModuleDict"),
    TorchSequential => ("torch.nn", "Sequential"),
    TorchModulesModule => ("torch.nn.modules.module", "Module"),
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use pyrefly_python::module::Module;
    use pyrefly_python::module_path::ModulePath;
    use pyrefly_python::nesting_context::NestingContext;
    use pyrefly_python::short_identifier::ShortIdentifier;
    use ruff_python_ast::Identifier;
    use ruff_python_ast::name::Name;
    use ruff_text_size::TextRange;

    use super::*;
    use crate::class::Class;
    use crate::class::ClassDefIndex;
    use crate::class::ClassType;
    use crate::class::PrecomputedTParams;
    use crate::test_type_env::TestTypeEnv;
    use crate::types::TArgs;

    #[test]
    fn test_lookup() {
        assert_eq!(
            KnownClass::lookup(ModuleName::builtins(), "int"),
            Some(KnownClass::Int)
        );
        assert_eq!(
            KnownClass::lookup(ModuleName::from_str("django.db.models.manager"), "Manager"),
            Some(KnownClass::DjangoManager)
        );
        assert_eq!(
            KnownClass::lookup(
                ModuleName::from_str("django.db.models.manager"),
                "BaseManager"
            ),
            Some(KnownClass::DjangoBaseManager)
        );
        assert_eq!(
            KnownClass::lookup(ModuleName::from_str("django.db.models.query"), "QuerySet"),
            Some(KnownClass::DjangoQuerySet)
        );
        assert_eq!(
            KnownClass::lookup(ModuleName::from_str("foo"), "int"),
            None,
            "the name alone does not make a known class"
        );
        assert_eq!(
            KnownClass::lookup(ModuleName::builtins(), "Foo"),
            None,
            "the module alone does not make a known class"
        );
    }

    #[test]
    fn test_top_level_class_is_known() {
        let env = TestTypeEnv::new();
        let int = env.class("builtins", "int");
        assert_eq!(int.known(), Some(KnownClass::Int));
        assert!(int.is(KnownClass::Int));
        assert!(!int.is(KnownClass::Str));
        let int_type = ClassType::new(int, TArgs::default());
        assert_eq!(int_type.known(), Some(KnownClass::Int));
        assert!(int_type.is(KnownClass::Int));
    }

    #[test]
    fn test_same_name_in_other_module_is_not_known() {
        let env = TestTypeEnv::new();
        let int = env.class("foo", "int");
        assert_eq!(int.known(), None);
        assert!(!int.is(KnownClass::Int));
    }

    #[test]
    fn test_nested_class_is_not_known() {
        let module = Module::new(
            ModuleName::builtins(),
            ModulePath::filesystem(PathBuf::from("builtins.py")),
            Arc::new(String::new()),
        );
        let outer = Identifier::new(Name::new("Foo"), TextRange::default());
        let nested = Class::new(
            ClassDefIndex(1),
            Identifier::new(Name::new("int"), TextRange::default()),
            NestingContext::class(ShortIdentifier::new(&outer), NestingContext::toplevel()),
            module,
            PrecomputedTParams::NotGeneric,
            false,
            None,
        );
        assert_eq!(
            nested.known(),
            None,
            "a class named `int` nested in a class in `builtins` is not `builtins.int`"
        );
    }
}
