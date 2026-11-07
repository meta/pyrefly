/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

//! A `TypeEnv` for unit tests, which defines its own classes.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

use dupe::Dupe;
use pyrefly_python::module::Module;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_python::nesting_context::NestingContext;
use pyrefly_python::sys_info::PythonVersion;
use ruff_python_ast::Identifier;
use ruff_python_ast::name::Name;
use ruff_text_size::TextRange;
use starlark_map::small_map::SmallMap;

use crate::class::Class;
use crate::class::ClassDefIndex;
use crate::class::ClassType;
use crate::class::PrecomputedTParams;
use crate::env::TypeEnv;
use crate::quantified::AnchorIndex;
use crate::quantified::Quantified;
use crate::quantified::QuantifiedIdentity;
use crate::quantified::QuantifiedKind;
use crate::quantified::QuantifiedOrigin;
use crate::stdlib::Stdlib;
use crate::type_var::PreInferenceVariance;
use crate::type_var::Restriction;
use crate::types::TArgs;
use crate::types::TParams;
use crate::types::Type;

/// The names of the type variables that typeshed declares for the generic standard library classes.
const STDLIB_TYPE_VARS: &[&str] = &[
    "_T",
    "_T_co",
    "_KT",
    "_VT",
    "_KT_co",
    "_VT_co",
    "_YieldT_co",
    "_SendT_contra",
    "_ReturnT_co",
    "_SendT_nd_contra",
    "_ReturnT_nd_co",
    "_StartT_co",
    "_StopT_co",
    "_StepT_co",
    "_BaseExceptionT_co",
    "_ExceptionT_co",
];

/// A `TypeEnv` for unit tests, which defines its own classes.
///
/// Its classes never compare equal to classes of real modules. A class's identity includes its
/// module's path, and every module here has a namespace package path. A namespace package has no
/// source, so the checker never defines a class in one.
///
/// Its `Stdlib` has every standard library class. The generic ones (`list`, `dict` and so on)
/// have the type parameters that typeshed declares for them.
pub struct TestTypeEnv {
    stdlib: Stdlib,
    /// Every module that has classes, with the number of classes defined in it so far. A new
    /// class gets the next index in its module, so no two classes defined here are equal.
    modules: RefCell<SmallMap<ModuleName, (Module, u32)>>,
}

impl TestTypeEnv {
    pub fn new() -> Self {
        // The env defines the stdlib's classes itself, so it starts with a placeholder stdlib.
        let mut env = Self {
            stdlib: Stdlib::for_bootstrapping(),
            modules: RefCell::new(SmallMap::new()),
        };
        let defined = RefCell::new(SmallMap::new());
        let stdlib = Stdlib::new(
            PythonVersion::default(),
            &|module, name| {
                let cls = env.stdlib_class(&defined, module, name);
                let tparams = env.class_tparams(&cls);
                let tparams = (!tparams.is_empty()).then(|| Arc::new(tparams.clone()));
                Some((cls, tparams))
            },
            &|_, _| None,
        );
        env.stdlib = stdlib;
        env
    }

    /// The standard library class `name` in `module`. `defined` holds the classes defined so
    /// far, so a class that a type parameter's bound refers to is the class that `Stdlib` holds.
    fn stdlib_class(
        &self,
        defined: &RefCell<SmallMap<(ModuleName, Name), Class>>,
        module: ModuleName,
        name: &Name,
    ) -> Class {
        if let Some(cls) = defined.borrow().get(&(module, name.clone())) {
            return cls.dupe();
        }
        let tparams = self.stdlib_tparams(defined, module, name);
        let cls = self.define_class(module, name.clone(), tparams);
        defined
            .borrow_mut()
            .insert((module, name.clone()), cls.dupe());
        cls
    }

    /// The type parameters that typeshed declares for the standard library class `name` in
    /// `module`, with their kinds, variances, bounds and defaults. Every class that `Stdlib` looks
    /// up and that is missing here is not generic.
    fn stdlib_tparams(
        &self,
        defined: &RefCell<SmallMap<(ModuleName, Name), Class>>,
        module: ModuleName,
        name: &Name,
    ) -> TParams {
        use PreInferenceVariance::Contravariant;
        use PreInferenceVariance::Covariant;
        use PreInferenceVariance::Invariant;
        // Like typeshed's module-level type variables, a type variable is identified by its
        // module and name, so classes that share one (`list` and `set` share `_T`) agree.
        let tparam = |name: &str, variance, bound: Option<Type>, default: Option<Type>| {
            let index = STDLIB_TYPE_VARS
                .iter()
                .position(|type_var| *type_var == name)
                .expect("every standard library type variable is in `STDLIB_TYPE_VARS`");
            Quantified::new(
                QuantifiedIdentity::new(
                    module,
                    AnchorIndex::new(TextRange::default(), index as u32),
                    QuantifiedOrigin::ScopedLegacy,
                ),
                Name::new(name),
                QuantifiedKind::TypeVar,
                default,
                bound.map_or(Restriction::Unrestricted, Restriction::Bound),
                variance,
            )
        };
        let builtin = |name: &'static str| {
            let cls = self.stdlib_class(defined, ModuleName::builtins(), &Name::new_static(name));
            ClassType::new(cls, TArgs::default()).to_type()
        };
        let quantified = |q: &Quantified| Type::Quantified(Box::new(q.clone()));
        let tparams = match name.as_str() {
            "list" | "set" | "enumerate" | "partial" | "deque" | "MutableSequence" => {
                vec![tparam("_T", Invariant, None, None)]
            }
            "tuple" | "frozenset" | "Iterable" | "Iterator" | "AsyncIterable" | "AsyncIterator"
            | "Sequence" | "Awaitable" => vec![tparam("_T_co", Covariant, None, None)],
            "dict" => vec![
                tparam("_KT", Invariant, None, None),
                tparam("_VT", Invariant, None, None),
            ],
            "Mapping" => vec![
                tparam("_KT", Invariant, None, None),
                tparam("_VT_co", Covariant, None, None),
            ],
            "dict_items" | "dict_keys" | "dict_values" => vec![
                tparam("_KT_co", Covariant, None, None),
                tparam("_VT_co", Covariant, None, None),
            ],
            "Generator" => vec![
                tparam("_YieldT_co", Covariant, None, None),
                tparam("_SendT_contra", Contravariant, None, Some(Type::None)),
                tparam("_ReturnT_co", Covariant, None, Some(Type::None)),
            ],
            "AsyncGenerator" => vec![
                tparam("_YieldT_co", Covariant, None, None),
                tparam("_SendT_contra", Contravariant, None, Some(Type::None)),
            ],
            "Coroutine" => vec![
                tparam("_YieldT_co", Covariant, None, None),
                tparam("_SendT_nd_contra", Contravariant, None, None),
                tparam("_ReturnT_nd_co", Covariant, None, None),
            ],
            "slice" => {
                let start = tparam("_StartT_co", Covariant, None, Some(Type::any_explicit()));
                let stop = tparam("_StopT_co", Covariant, None, Some(quantified(&start)));
                let step = tparam(
                    "_StepT_co",
                    Covariant,
                    None,
                    Some(Type::union(vec![quantified(&start), quantified(&stop)])),
                );
                vec![start, stop, step]
            }
            "BaseExceptionGroup" => {
                let base_exception = builtin("BaseException");
                vec![tparam(
                    "_BaseExceptionT_co",
                    Covariant,
                    Some(base_exception.clone()),
                    Some(base_exception),
                )]
            }
            "ExceptionGroup" => {
                let exception = builtin("Exception");
                vec![tparam(
                    "_ExceptionT_co",
                    Covariant,
                    Some(exception.clone()),
                    Some(exception),
                )]
            }
            _ => Vec::new(),
        };
        TParams::new(tparams)
    }

    /// Define a non-generic class `name` at the top level of `module`.
    pub fn class(&self, module: &str, name: &str) -> Class {
        self.generic_class(module, name, TParams::empty())
    }

    /// Define a class `name` with type parameters `tparams` at the top level of `module`.
    pub fn generic_class(&self, module: &str, name: &str, tparams: TParams) -> Class {
        self.define_class(ModuleName::from_str(module), Name::new(name), tparams)
    }

    fn define_class(&self, module_name: ModuleName, name: Name, tparams: TParams) -> Class {
        let mut modules = self.modules.borrow_mut();
        let (module, count) = modules.entry(module_name).or_insert_with(|| {
            let path = ModulePath::namespace(PathBuf::from(module_name.as_str()));
            (Module::new(module_name, path, Arc::new(String::new())), 0)
        });
        let index = ClassDefIndex(*count);
        *count += 1;
        let tparams = if tparams.is_empty() {
            PrecomputedTParams::NotGeneric
        } else {
            PrecomputedTParams::Precomputed(Arc::new(tparams))
        };
        Class::new(
            index,
            Identifier::new(name, TextRange::default()),
            NestingContext::toplevel(),
            module.dupe(),
            tparams,
            false,
            None,
        )
    }
}

impl TypeEnv for TestTypeEnv {
    fn stdlib(&self) -> &Stdlib {
        &self.stdlib
    }

    fn class_tparams<'a>(&'a self, cls: &'a Class) -> &'a TParams {
        match cls.precomputed_tparams() {
            PrecomputedTParams::NotGeneric => TParams::empty_ref(),
            PrecomputedTParams::Precomputed(tparams) => tparams,
            PrecomputedTParams::FromBinding => {
                unreachable!("`TestTypeEnv` defines every class with precomputed type parameters")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::class::ClassKind;
    use crate::heap::TypeHeap;
    use crate::known_class::KnownClass;

    fn type_var(name: &str, restriction: Restriction, default: Option<Type>) -> Quantified {
        Quantified::new(
            QuantifiedIdentity::new(
                ModuleName::from_str("foo"),
                AnchorIndex::first(TextRange::default()),
                QuantifiedOrigin::Pep695,
            ),
            Name::new(name),
            QuantifiedKind::TypeVar,
            default,
            restriction,
            PreInferenceVariance::Covariant,
        )
    }

    #[test]
    fn test_classes_are_distinct() {
        let env = TestTypeEnv::new();
        let foo_bar = env.class("foo", "Bar");
        let foo_baz = env.class("foo", "Baz");
        let qux_bar = env.class("qux", "Bar");
        assert_ne!(
            foo_bar, foo_baz,
            "classes in one module get different indices"
        );
        assert_ne!(
            foo_bar, qux_bar,
            "same-named classes in different modules differ"
        );
    }

    #[test]
    fn test_classes_differ_from_real_classes() {
        let env = TestTypeEnv::new();
        let fake = env.class("foo", "Bar");
        let real_module = |path: ModulePath| {
            Module::new(ModuleName::from_str("foo"), path, Arc::new(String::new()))
        };
        for path in [
            ModulePath::filesystem(PathBuf::from("foo")),
            ModulePath::memory(PathBuf::from("foo")),
        ] {
            let real = Class::new(
                fake.index(),
                Identifier::new(Name::new("Bar"), TextRange::default()),
                NestingContext::toplevel(),
                real_module(path),
                PrecomputedTParams::NotGeneric,
                false,
                None,
            );
            assert_ne!(
                fake, real,
                "a real class with the same module name, path, index and name is a different class"
            );
        }
    }

    #[test]
    fn test_class_def_reads() {
        let env = TestTypeEnv::new();
        let cls = env.class("foo", "Bar");
        // Read through a trait object to check that `TypeEnv` is object safe.
        let env: &dyn TypeEnv = &env;
        assert!(std::ptr::eq(cls.def_in(env), cls.inline_def()));
        assert_eq!(cls.name_in(env).as_str(), "Bar");
        assert_eq!(cls.qname_in(env).id().as_str(), "Bar");
        assert_eq!(cls.module_in(env).name(), ModuleName::from_str("foo"));
        assert_eq!(cls.range_in(env), TextRange::default());
        assert!(!cls.is_protocol_in(env));
        assert!(cls.deprecation_in(env).is_none());
        assert_eq!(cls.kind_in(env), ClassKind::Class);
        assert!(cls.tparams_in(env).is_empty());
        assert_eq!(cls.known(), None, "`foo.Bar` is not a known class");
    }

    #[test]
    fn test_class_tparams() {
        let env = TestTypeEnv::new();
        let t = type_var("T", Restriction::Unrestricted, None);
        let tparams = TParams::new(vec![t]);
        let cls = env.generic_class("foo", "Box", tparams.clone());
        assert_eq!(cls.tparams_in(&env), &tparams);

        let class_type = ClassType::new(
            cls,
            TArgs::new(Arc::new(tparams.clone()), vec![Type::any_implicit()]),
        );
        assert_eq!(class_type.tparams_in(&env), &tparams);
        assert_eq!(class_type.name_in(&env).as_str(), "Box");
    }

    #[test]
    fn test_stdlib_classes() {
        let env = TestTypeEnv::new();
        let int = env.stdlib().int();
        assert_eq!(int.name_in(&env).as_str(), "int");
        assert!(int.is(KnownClass::Int));
        assert!(int.tparams_in(&env).is_empty());
        assert_ne!(
            env.stdlib().int().class_object(),
            env.stdlib().str().class_object(),
            "stdlib classes in one module are distinct"
        );
        assert_ne!(
            env.class("builtins", "int"),
            *int.class_object(),
            "a class defined later continues the module's indices"
        );
    }

    #[test]
    fn test_quantified_reads() {
        let env = TestTypeEnv::new();
        let int = env.stdlib().int().clone().to_type();
        let t = type_var("T", Restriction::Bound(int.clone()), Some(int.clone()));
        assert_eq!(t.name_in(&env).as_str(), "T");
        assert_eq!(t.restriction_in(&env), &Restriction::Bound(int.clone()));
        assert_eq!(t.default_in(&env), Some(&int));
        assert_eq!(t.variance_in(&env), PreInferenceVariance::Covariant);

        let u = type_var("U", Restriction::Unrestricted, None);
        assert_eq!(u.restriction_in(&env), &Restriction::Unrestricted);
        assert_eq!(u.default_in(&env), None);
    }

    #[test]
    fn test_stdlib_is_complete() {
        // Each accessor panics if its class is missing or has the wrong number of type
        // parameters.
        let env = TestTypeEnv::new();
        let stdlib = env.stdlib();
        let heap = TypeHeap::new();
        let x = || Type::None;
        stdlib.object();
        stdlib.bool();
        stdlib.builtins_type();
        stdlib.enum_meta();
        stdlib.protocol_meta();
        stdlib.enum_flag();
        stdlib.enum_class();
        stdlib.named_tuple_fallback();
        stdlib.typed_dict_fallback();
        stdlib.ellipsis_type();
        stdlib.none_type();
        stdlib.int();
        stdlib.float();
        stdlib.complex();
        stdlib.decimal();
        stdlib.bytes();
        stdlib.bytearray();
        stdlib.date();
        stdlib.datetime();
        stdlib.time();
        stdlib.timedelta();
        stdlib.path();
        stdlib.uuid();
        stdlib.str();
        stdlib.slice_class_object();
        stdlib.base_exception();
        stdlib.base_exception_group(x());
        stdlib.exception_group(x());
        stdlib.exception_group_object();
        stdlib.union_type();
        stdlib.tuple_object();
        stdlib.tuple(x());
        stdlib.enumerate(x());
        stdlib.list(x());
        stdlib.partial(x());
        stdlib.list_object();
        stdlib.deque(x());
        stdlib.frozenset(x());
        stdlib.frozenset_object();
        stdlib.dict(x(), x());
        stdlib.dict_object();
        stdlib.dict_items(x(), x());
        stdlib.dict_keys(x(), x());
        stdlib.dict_values(x(), x());
        stdlib.mapping(x(), x());
        stdlib.mapping_object();
        stdlib.set(x());
        stdlib.set_object();
        stdlib.iterable(x());
        stdlib.iterator(x());
        stdlib.async_iterable(x());
        stdlib.async_iterator(x());
        stdlib.mutable_sequence(x());
        stdlib.sequence(x());
        stdlib.generator(x(), x(), x());
        stdlib.async_generator(x(), x());
        stdlib.awaitable(x());
        stdlib.awaitable_object();
        stdlib.coroutine(x(), x(), x());
        stdlib.coroutine_object();
        stdlib.type_var();
        stdlib.param_spec();
        stdlib.type_var_tuple();
        stdlib.param_spec_args();
        stdlib.param_spec_kwargs();
        stdlib.param_spec_args_as_tuple(&heap);
        stdlib.param_spec_kwargs_as_dict(&heap);
        stdlib.type_alias_type();
        stdlib.sentinel();
        stdlib.traceback_type();
        stdlib.function_type();
        stdlib.method_type();
        stdlib.module_type();
        stdlib.property();
        stdlib.generic_alias();
        stdlib.template();
    }
}
