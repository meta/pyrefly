/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::cmp::Ord;
use std::cmp::Ordering;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use dupe::Dupe;
use parse_display::Display;
use pyrefly_derive::TypeEq;
use pyrefly_derive::Visit;
use pyrefly_derive::VisitMut;
use pyrefly_python::module::Module;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_python::nesting_context::NestingContext;
use pyrefly_python::qname::QName;
use pyrefly_util::visit::Visit;
use pyrefly_util::visit::VisitMut;
use ruff_python_ast::Identifier;
use ruff_python_ast::name::Name;
use ruff_text_size::TextRange;
use starlark_map::small_map::SmallMap;

use crate::env::TypeEnv;
use crate::equality::TypeEq;
use crate::equality::TypeEqCtx;
use crate::function::Deprecation;
use crate::known_class::KnownClass;
use crate::types::Substitution;
use crate::types::TArgs;
use crate::types::TParams;
use crate::types::Type;

/// The name of a nominal type, e.g. `str`
#[derive(Debug, Clone, Display, Dupe)]
pub struct Class(Arc<ClassDef>);

impl Hash for Class {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key_eq().hash(state)
    }
}

impl PartialEq for Class {
    fn eq(&self, other: &Self) -> bool {
        self.key_eq().eq(&other.key_eq())
    }
}

impl Eq for Class {}
// Deprecation changes invalidate cached references without changing nominal class identity.
impl TypeEq for Class {
    fn type_eq(&self, other: &Self, _ctx: &mut TypeEqCtx) -> bool {
        self == other && self.deprecation() == other.deprecation()
    }
}

impl Ord for Class {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key_ord().cmp(&other.key_ord())
    }
}

impl PartialOrd for Class {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// There are types stored inside Class, in the TParams, but we don't want to visit them.
// We typically visit types to get rid of Var, and promise these are not interesting in that sense.
impl VisitMut<Type> for Class {
    fn recurse_mut(&mut self, _: &mut dyn FnMut(&mut Type)) {}
}
impl Visit<Type> for Class {
    fn recurse<'a>(&'a self, _: &mut dyn FnMut(&'a Type)) {}
}

/// `attr.ib`/`attrib` honor a `type=` argument and accept a positional default; `field` neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrsFieldSpecifierKind {
    Attrib,
    Field,
}

/// Bundling these with the specifier keeps them unrepresentable without one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrsFieldSpecifier {
    pub kind: AttrsFieldSpecifierKind,
    pub default_is_nothing: bool,
    pub default_decorator_method_range: Option<TextRange>,
    pub converter_decorator_method_range: Option<TextRange>,
}

/// Simple properties of class fields that can be attached to the class definition. Note that this
/// does not include the type of a field, which needs to be computed lazily to avoid a recursive loop.
#[derive(Debug, Clone)]
pub struct ClassFieldProperties {
    is_annotated: bool,
    // The field is initialized on the class (outside of a method)
    is_initialized_on_class: bool,
    // The field is defined in the class body (not in a method via self.x = ...)
    is_defined_in_class_body: bool,
    attrs_field_specifier: Option<AttrsFieldSpecifier>,
    /// An explicit Django method-copy policy set in the class body.
    django_queryset_only: Option<bool>,
    range: TextRange,
    // The range of the docstring following this field, if present
    docstring_range: Option<TextRange>,
}

/// The index of a class within the file, used as a reference to data associated with the class.
#[derive(Debug, Clone, Dupe, Copy, Eq, PartialEq, Hash, PartialOrd, Ord)]
#[derive(Display)]
pub struct ClassDefIndex(pub u32);

impl ClassFieldProperties {
    pub fn new(
        is_annotated: bool,
        has_default_value: bool,
        is_defined_in_class_body: bool,
        attrs_field_specifier: Option<AttrsFieldSpecifier>,
        range: TextRange,
        docstring_range: Option<TextRange>,
    ) -> Self {
        Self {
            is_annotated,
            is_initialized_on_class: has_default_value,
            is_defined_in_class_body,
            attrs_field_specifier,
            django_queryset_only: None,
            range,
            docstring_range,
        }
    }

    pub fn is_initialized_on_class(&self) -> bool {
        self.is_initialized_on_class
    }

    /// Attach the method-copy policy found during binding.
    pub fn with_django_queryset_only(mut self, value: Option<bool>) -> Self {
        self.django_queryset_only = value;
        self
    }
    pub fn is_defined_in_class_body(&self) -> bool {
        self.is_defined_in_class_body
    }

    pub fn docstring_range(&self) -> Option<TextRange> {
        self.docstring_range
    }
}

/// The set of fields declared on a class, with their properties.
#[derive(Clone, Debug, Default)]
pub struct ClassFields(SmallMap<Name, ClassFieldProperties>);

impl ClassFields {
    pub fn new(fields: SmallMap<Name, ClassFieldProperties>) -> Self {
        Self(fields)
    }

    pub fn empty() -> Self {
        Self(SmallMap::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, name: &Name) -> bool {
        self.0.contains_key(name)
    }

    pub fn get_index_of(&self, name: &Name) -> Option<usize> {
        self.0.get_index_of(name)
    }

    pub fn fields(&self) -> impl ExactSizeIterator<Item = &Name> {
        self.0.keys()
    }

    /// Alias for `fields()` — iterates over field names.
    pub fn names(&self) -> impl ExactSizeIterator<Item = &Name> {
        self.0.keys()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Name, &ClassFieldProperties)> {
        self.0.iter()
    }

    pub fn class_body_fields(&self) -> impl Iterator<Item = &Name> {
        self.0
            .iter()
            .filter(|(_, prop)| prop.is_defined_in_class_body)
            .map(|(name, _)| name)
    }

    pub fn is_field_annotated(&self, name: &Name) -> bool {
        self.0.get(name).is_some_and(|prop| prop.is_annotated)
    }

    /// Return the explicit Django method-copy policy, if present.
    pub fn django_queryset_only(&self, name: &Name) -> Option<bool> {
        self.0.get(name).and_then(|prop| prop.django_queryset_only)
    }

    pub fn is_attrs_field_specifier(&self, name: &Name) -> bool {
        self.0
            .get(name)
            .is_some_and(|prop| prop.attrs_field_specifier.is_some())
    }

    pub fn default_is_attrs_nothing(&self, name: &Name) -> bool {
        self.0.get(name).is_some_and(|prop| {
            prop.attrs_field_specifier
                .is_some_and(|s| s.default_is_nothing)
        })
    }

    pub fn default_is_attrs_decorator(&self, name: &Name) -> bool {
        self.attrs_default_decorator_method_range(name).is_some()
    }

    /// The name range of this field's `@<field>.default` method, if any.
    pub fn attrs_default_decorator_method_range(&self, name: &Name) -> Option<TextRange> {
        self.0
            .get(name)
            .and_then(|prop| prop.attrs_field_specifier)
            .and_then(|s| s.default_decorator_method_range)
    }

    /// The name range of this field's first `@<field>.converter` method, if any.
    pub fn attrs_converter_decorator_method_range(&self, name: &Name) -> Option<TextRange> {
        self.0
            .get(name)
            .and_then(|prop| prop.attrs_field_specifier)
            .and_then(|s| s.converter_decorator_method_range)
    }

    /// Whether the field's attrs specifier honors a `type=` argument (`attr.ib`, not `field`).
    pub fn attrs_specifier_honors_type(&self, name: &Name) -> bool {
        self.0.get(name).is_some_and(|prop| {
            prop.attrs_field_specifier
                .is_some_and(|s| s.kind == AttrsFieldSpecifierKind::Attrib)
        })
    }

    pub fn is_field_initialized_on_class(&self, name: &Name) -> bool {
        self.0
            .get(name)
            .is_some_and(|prop| prop.is_initialized_on_class)
    }

    pub fn field_decl_range(&self, name: &Name) -> Option<TextRange> {
        Some(self.0.get(name)?.range)
    }

    pub fn field_docstring_range(&self, name: &Name) -> Option<TextRange> {
        self.0.get(name)?.docstring_range
    }
}

/// What is known about a class's type parameters after the binding phase.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrecomputedTParams {
    /// The class declares no type parameters, so there is nothing to look up.
    NotGeneric,
    /// The class has legacy type variables. Computing its type parameters at
    /// binding time could produce a cycle, so they come from the class's
    /// `KeyTParams` / `BindingTParams` pair instead.
    FromBinding,
    /// Computed during binding, which is possible whenever the class has no
    /// legacy type variables.
    Precomputed(Arc<TParams>),
}

/// The data of a class definition.
///
/// Align to a cache line to prevent false sharing. `ClassDef` is stored
/// behind `Arc` and accessed concurrently from multiple threads during
/// type checking. Without alignment, multiple `Arc<ClassDef>` allocations
/// can land on the same cache line, causing cross-thread invalidation
/// traffic when any of them are accessed (even read-only, due to the Arc
/// refcount on adjacent allocations).
#[repr(align(64))]
pub struct ClassDef {
    def_index: ClassDefIndex,
    /// Only set for top-level classes.
    known: Option<KnownClass>,
    qname: QName,
    is_protocol: bool,
    deprecation: Option<Deprecation>,
    precomputed_tparams: PrecomputedTParams,
}

impl Debug for ClassDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClassDef")
            .field("index", &self.def_index)
            .field("qname", &self.qname)
            .field("is_protocol", &self.is_protocol)
            .field("deprecation", &self.deprecation)
            .field("tparams", &self.precomputed_tparams)
            // We don't print `fields` because it's way too long.
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Visit, VisitMut, TypeEq)]
pub enum ClassKind {
    StaticMethod(Name),
    ClassMethod(Name),
    Property(Name),
    CachedProperty(Name),
    Class,
    EnumMember,
    EnumNonmember,
    DataclassField,
}

impl ClassKind {
    /// `name` is the class's own name, which the descriptor kinds carry.
    fn new(known: Option<KnownClass>, name: &Name) -> Self {
        match known {
            Some(KnownClass::StaticMethod | KnownClass::AbcAbstractStaticMethod) => {
                Self::StaticMethod(name.clone())
            }
            Some(KnownClass::ClassMethod | KnownClass::AbcAbstractClassMethod) => {
                Self::ClassMethod(name.clone())
            }
            Some(
                KnownClass::Property | KnownClass::AbcAbstractProperty | KnownClass::EnumProperty,
            ) => Self::Property(name.clone()),
            Some(
                KnownClass::FunctoolsCachedProperty
                | KnownClass::CachedPropertyThreadedCachedProperty
                | KnownClass::CachedProperty
                | KnownClass::CinderCachedProperty
                | KnownClass::CinderAsyncCachedProperty,
            ) => Self::CachedProperty(name.clone()),
            Some(KnownClass::EnumMember) => Self::EnumMember,
            Some(KnownClass::EnumNonmember) => Self::EnumNonmember,
            Some(KnownClass::DataclassesField) => Self::DataclassField,
            _ => Self::Class,
        }
    }
}

impl Display for ClassDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "class {}", self.qname.id())?;
        writeln!(f, ": ...")
    }
}

// A note on terminology regarding attribute-related concepts:
// - "field" refers to something defined in a class body, with a raw type as written.
// - "member" refers to a name defined on a class, including inherited members whose
//   types should be expressed in terms of the type parameters of the current class
// - "attribute" refers to a value actually accessed from an instance or class object,
//   which involves substituting type arguments for the class type parameters as
//   well as descriptor handling (including method binding).
impl Class {
    pub fn new(
        def_index: ClassDefIndex,
        name: Identifier,
        parent: NestingContext,
        module: Module,
        precomputed_tparams: PrecomputedTParams,
        is_protocol: bool,
        deprecation: Option<Deprecation>,
    ) -> Self {
        let known = if parent.is_toplevel() {
            KnownClass::lookup(module.name(), name.id.as_str())
        } else {
            None
        };
        Self(Arc::new(ClassDef {
            def_index,
            known,
            qname: QName::new(name, parent, module),
            is_protocol,
            deprecation,
            precomputed_tparams,
        }))
    }

    pub fn range(&self) -> TextRange {
        self.0.qname.range()
    }

    pub fn name(&self) -> &Name {
        self.0.qname.id()
    }

    pub fn qname(&self) -> &QName {
        &self.0.qname
    }

    pub fn kind(&self) -> ClassKind {
        ClassKind::new(self.known(), self.name())
    }

    pub fn is_protocol(&self) -> bool {
        self.0.is_protocol
    }

    pub fn deprecation(&self) -> Option<&Deprecation> {
        self.0.deprecation.as_ref()
    }

    pub fn precomputed_tparams(&self) -> &PrecomputedTParams {
        &self.0.precomputed_tparams
    }

    pub fn index(&self) -> ClassDefIndex {
        self.0.def_index
    }

    pub fn module_name(&self) -> ModuleName {
        self.0.qname.module_name()
    }

    pub fn module_path(&self) -> &ModulePath {
        self.0.qname.module_path()
    }

    pub fn module(&self) -> &Module {
        self.0.qname.module()
    }

    /// The known class this is, if it is one. Only top-level classes can be known classes.
    pub fn known(&self) -> Option<KnownClass> {
        self.0.known
    }

    pub fn is(&self, known: KnownClass) -> bool {
        self.0.known == Some(known)
    }

    /// The definition data stored in this `Class`, which `TypeEnv::class_def` returns by default.
    pub(crate) fn inline_def(&self) -> &ClassDef {
        &self.0
    }

    pub fn def_in<'a, E: TypeEnv + ?Sized>(&'a self, env: &'a E) -> &'a ClassDef {
        env.class_def(self)
    }

    pub fn name_in<'a, E: TypeEnv + ?Sized>(&'a self, env: &'a E) -> &'a Name {
        self.def_in(env).qname.id()
    }

    pub fn qname_in<'a, E: TypeEnv + ?Sized>(&'a self, env: &'a E) -> &'a QName {
        &self.def_in(env).qname
    }

    pub fn range_in<E: TypeEnv + ?Sized>(&self, env: &E) -> TextRange {
        self.def_in(env).qname.range()
    }

    /// The module that defines this class.
    pub fn module_in<'a, E: TypeEnv + ?Sized>(&'a self, env: &'a E) -> &'a Module {
        self.def_in(env).qname.module()
    }

    pub fn is_protocol_in<E: TypeEnv + ?Sized>(&self, env: &E) -> bool {
        self.def_in(env).is_protocol
    }

    pub fn deprecation_in<'a, E: TypeEnv + ?Sized>(
        &'a self,
        env: &'a E,
    ) -> Option<&'a Deprecation> {
        self.def_in(env).deprecation.as_ref()
    }

    pub fn kind_in<E: TypeEnv + ?Sized>(&self, env: &E) -> ClassKind {
        ClassKind::new(self.known(), self.name_in(env))
    }

    /// The type parameters of this class, which are empty if it is not generic.
    pub fn tparams_in<'a, E: TypeEnv + ?Sized>(&'a self, env: &'a E) -> &'a TParams {
        env.class_tparams(self)
    }

    /// Key to use for equality purposes. If we have the same module and index,
    /// we must point at the same class underneath.
    fn key_eq(&self) -> (ClassDefIndex, ModuleName, ModulePath) {
        (
            self.0.def_index,
            self.0.qname.module_name(),
            self.0.qname.module_path().to_key_eq(),
        )
    }

    /// Key to use for comparison purposes. Main used to sort identifiers in union,
    /// and then alphabetically sorting by the name gives a predictable answer.
    fn key_ord(&self) -> (&QName, ClassDefIndex) {
        (&self.0.qname, self.0.def_index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Visit, VisitMut, TypeEq)]
pub struct ClassType(Class, TArgs, Option<Arc<DjangoClassMetadata>>);

/// Construction-time Django specializations on nominal class instances.
///
/// The manager resolves generated members against its queryset. A specialized class uses
/// its effective base for inherited members and nominal upcasts. Type traversal and substitution
/// include this data, and instance binding preserves it when replacing `Self`. Explicit method
/// return types stand alone: neither their model nor their row is rebound to the manager's model.
/// Fields retain construction-specific accessor and override contracts.
/// A many-to-many manager records the target model's primary key type, which replaces the
/// `int` that the stubs accept in place of model instances.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(TypeEq)]
pub enum DjangoClassMetadata {
    Manager {
        queryset: ClassType,
    },
    ManyRelatedManager {
        pk_type: Type,
    },
    SpecializedBase {
        base: ClassType,
    },
    Field {
        get_type: Type,
        set_type: Type,
        override_type: Type,
    },
}

impl Visit<Type> for Arc<DjangoClassMetadata> {
    fn recurse<'a>(&'a self, f: &mut dyn FnMut(&'a Type)) {
        match self.as_ref() {
            DjangoClassMetadata::Manager { queryset } => queryset.visit(f),
            DjangoClassMetadata::ManyRelatedManager { pk_type } => f(pk_type),
            DjangoClassMetadata::SpecializedBase { base } => base.visit(f),
            DjangoClassMetadata::Field {
                get_type,
                set_type,
                override_type,
            } => {
                f(get_type);
                f(set_type);
                f(override_type);
            }
        }
    }
}

impl VisitMut<Type> for Arc<DjangoClassMetadata> {
    fn recurse_mut(&mut self, f: &mut dyn FnMut(&mut Type)) {
        match Arc::make_mut(self) {
            DjangoClassMetadata::Manager { queryset } => queryset.visit_mut(f),
            DjangoClassMetadata::ManyRelatedManager { pk_type } => f(pk_type),
            DjangoClassMetadata::SpecializedBase { base } => base.visit_mut(f),
            DjangoClassMetadata::Field {
                get_type,
                set_type,
                override_type,
            } => {
                f(get_type);
                f(set_type);
                f(override_type);
            }
        }
    }
}

impl Display for ClassType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Type::ClassType(self.clone()))
    }
}

impl ClassType {
    /// Create a class type.
    /// The `targs` must match the `tparams`, if this fails we will panic.
    pub fn new(class: Class, targs: TArgs) -> Self {
        Self(class, targs, None)
    }

    pub fn class_object(&self) -> &Class {
        &self.0
    }

    pub fn into_class_object(self) -> Class {
        self.0
    }

    pub fn tparams(&self) -> &TParams {
        self.1.tparams()
    }

    pub fn targs(&self) -> &TArgs {
        &self.1
    }

    pub fn targs_mut(&mut self) -> &mut TArgs {
        &mut self.1
    }

    /// Rewrite type arguments of some class relative to another.
    ///
    /// This is used to propagate instantiation of base class type parameters when computing
    /// the MRO.
    pub fn substitute_with(&self, substitution: &Substitution) -> Self {
        let mut result = self.clone();
        result.1 = self.1.substitute_with(substitution);
        if let Some(metadata) = &mut result.2 {
            metadata.recurse_mut(&mut |ty| substitution.substitute_into_mut(ty));
        }
        result
    }

    /// Attach a Django specialization to this nominal class instance.
    pub fn with_django_metadata(mut self, metadata: DjangoClassMetadata) -> Self {
        self.2 = Some(Arc::new(metadata));
        self
    }

    /// Return the construction-time Django specialization, if present.
    pub fn django_metadata(&self) -> Option<&DjangoClassMetadata> {
        self.2.as_deref()
    }

    pub fn substitution(&self) -> Substitution<'_> {
        self.targs().substitution()
    }

    pub fn name(&self) -> &Name {
        self.0.name()
    }

    pub fn qname(&self) -> &QName {
        self.0.qname()
    }

    pub fn to_type(self) -> Type {
        Type::ClassType(self)
    }

    pub fn known(&self) -> Option<KnownClass> {
        self.0.known()
    }

    pub fn is(&self, known: KnownClass) -> bool {
        self.0.is(known)
    }

    pub fn def_in<'a, E: TypeEnv + ?Sized>(&'a self, env: &'a E) -> &'a ClassDef {
        self.0.def_in(env)
    }

    pub fn name_in<'a, E: TypeEnv + ?Sized>(&'a self, env: &'a E) -> &'a Name {
        self.0.name_in(env)
    }

    pub fn qname_in<'a, E: TypeEnv + ?Sized>(&'a self, env: &'a E) -> &'a QName {
        self.0.qname_in(env)
    }

    pub fn range_in<E: TypeEnv + ?Sized>(&self, env: &E) -> TextRange {
        self.0.range_in(env)
    }

    pub fn module_in<'a, E: TypeEnv + ?Sized>(&'a self, env: &'a E) -> &'a Module {
        self.0.module_in(env)
    }

    pub fn is_protocol_in<E: TypeEnv + ?Sized>(&self, env: &E) -> bool {
        self.0.is_protocol_in(env)
    }

    pub fn deprecation_in<'a, E: TypeEnv + ?Sized>(
        &'a self,
        env: &'a E,
    ) -> Option<&'a Deprecation> {
        self.0.deprecation_in(env)
    }

    pub fn kind_in<E: TypeEnv + ?Sized>(&self, env: &E) -> ClassKind {
        self.0.kind_in(env)
    }

    /// The type parameters of the class, which are empty if it is not generic.
    pub fn tparams_in<'a, E: TypeEnv + ?Sized>(&'a self, env: &'a E) -> &'a TParams {
        self.0.tparams_in(env)
    }
}
