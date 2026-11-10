/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use pyrefly_config::error_kind::ErrorKind;
use pyrefly_python::ast::Ast;
use pyrefly_python::dunder;
use pyrefly_types::annotation::Annotation;
use pyrefly_types::class::ClassType;
use pyrefly_types::known_class::KnownClass;
use pyrefly_types::literal::LitEnum;
use pyrefly_types::read_only::ReadOnlyReason;
use ruff_python_ast::helpers::is_dunder;
use ruff_python_ast::helpers::is_sunder;
use ruff_python_ast::name::Name;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use starlark_map::small_set::SmallSet;

use crate::alt::answers::LookupAnswer;
use crate::alt::answers_solver::AnswersSolver;
use crate::alt::call::ConstructorKind;
use crate::alt::callable::CallArg;
use crate::alt::callable::CallKeyword;
use crate::alt::class::class_field::ClassAttribute;
use crate::alt::class::django::transform_django_enum_value;
use crate::alt::types::class_metadata::ClassMetadata;
use crate::alt::types::class_metadata::EnumMetadata;
use crate::binding::binding::ClassFieldDefinition;
use crate::error::collector::ErrorCollector;
use crate::error::context::ErrorContext;
use crate::error::context::TypeCheckContext;
use crate::error::context::TypeCheckKind;
use crate::types::class::Class;
use crate::types::literal::Lit;
use crate::types::types::Type;

/// The `_value_` attribute in enums is reserved, and can be annotated to
/// indicate an explicit type restriction on enum members. Looking it up
/// on an enum member will give the raw value of that member.
pub const VALUE: Name = Name::new_static("_value_");
/// The `value` attribute of an enum is a property that returns `_value_`.
pub const VALUE_PROP: Name = Name::new_static("value");

pub const GENERATE_NEXT_VALUE: Name = Name::new_static("_generate_next_value_");

#[derive(Debug)]
pub(crate) struct EnumLookupArg {
    pub ty: Type,
    pub range: TextRange,
    pub is_positional: bool,
}

impl<'ctx, 'answer, Ans: LookupAnswer> AnswersSolver<'ctx, 'answer, Ans> {
    /// Suggest an enum member when a raw literal matches exactly one expected enum value.
    pub fn suggest_enum_member_for_value(&self, got: &Type, want: &Type) -> Option<String> {
        match want {
            Type::ClassType(cls) | Type::SelfType(cls) => {
                self.suggest_enum_member_for_class_value(cls.class_object(), got)
            }
            Type::Literal(lit) => match &lit.value {
                Lit::Enum(lit_enum) => {
                    self.suggest_enum_member_for_class_value(lit_enum.class.class_object(), got)
                }
                _ => None,
            },
            Type::Union(f) => {
                let mut suggestion = None;
                for member in &f.members {
                    if let Some(candidate) = self.suggest_enum_member_for_value(got, member) {
                        match &suggestion {
                            Some(existing) if existing != &candidate => return None,
                            _ => suggestion = Some(candidate),
                        }
                    }
                }
                suggestion
            }
            _ => None,
        }
    }

    pub fn get_enum_member(&self, cls: &Class, name: &Name) -> Option<Lit> {
        self.get_field_from_current_class_only(cls, name)
            .and_then(|field| self.as_enum_member(field, cls))
    }

    pub fn get_enum_members(&self, cls: &Class) -> SmallSet<Lit> {
        self.get_class_fields(cls)
            .map(|class_fields| {
                class_fields
                    .names()
                    .filter_map(|f| self.get_enum_member(cls, f))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn suggest_enum_member_for_class_value(&self, cls: &Class, got: &Type) -> Option<String> {
        let is_django = self.get_metadata_for_class(cls).enum_metadata()?.is_django;
        let mut suggestion = None;
        for lit in self.get_enum_members(cls) {
            let Lit::Enum(lit_enum) = lit else {
                unreachable!("enum members must be represented as enum literals");
            };
            let value_ty = self.enum_literal_to_value_type((*lit_enum).clone(), is_django);
            if self.is_equivalent(got, &value_ty) {
                let candidate = format!("{}.{}", lit_enum.class.name(), lit_enum.member);
                match &suggestion {
                    Some(existing) if existing != &candidate => return None,
                    _ => suggestion = Some(candidate),
                }
            }
        }
        suggestion
    }

    fn is_valid_enum_member(
        &self,
        name: &Name,
        ty: &Type,
        field_definition: &ClassFieldDefinition,
    ) -> bool {
        // Names starting but not ending with __ are private.
        // Names starting and ending with _ are reserved by the enum.
        if Ast::is_mangled_attr(name) || is_sunder(name.as_str()) || is_dunder(name.as_str()) {
            return false;
        }
        // Methods decorated with @enum.member are always enum members.
        if ty
            .toplevel_func_metadata()
            .is_some_and(|meta| meta.flags.has_enum_member_decoration)
        {
            return true;
        }
        // Only values assigned or defined in the class body can be enum members.
        // MethodLike definitions (def statements) are not enum member candidates
        // unless decorated with @member (handled above).
        match field_definition {
            ClassFieldDefinition::AssignedInBody { .. }
            | ClassFieldDefinition::DefinedWithoutAssign { .. } => {}
            _ => return false,
        }
        match ty {
            // Callables are not valid enum members.
            _ if ty.is_toplevel_callable() => false,
            // Values initialized with nonmember() or descriptor-like wrappers are not members.
            Type::ClassType(cls)
                if cls.is(KnownClass::EnumNonmember)
                    || cls.is(KnownClass::StaticMethod)
                    || cls.is(KnownClass::ClassMethod)
                    || cls.is(KnownClass::TypesDynamicClassAttribute)
                    || cls.is(KnownClass::EnumProperty) =>
            {
                false
            }
            _ => true,
        }
    }

    /// Checks for a special-cased enum attribute, falling back to a regular instance attribute lookup.
    pub fn get_enum_or_instance_attribute(
        &self,
        class: &ClassType,
        metadata: &ClassMetadata,
        attr_name: &Name,
    ) -> Option<ClassAttribute> {
        self.special_case_enum_attr_lookup(class, None, metadata, attr_name)
            .or_else(|| self.get_instance_attribute(class, attr_name))
    }

    /// Checks for a special-cased enum attribute on an enum literal, falling back to a regular instance attribute lookup.
    pub fn get_enum_literal_or_instance_attribute(
        &self,
        lit: &LitEnum,
        metadata: &ClassMetadata,
        attr_name: &Name,
    ) -> Option<ClassAttribute> {
        let class = &lit.class;
        self.special_case_enum_attr_lookup(class, Some(lit), metadata, attr_name)
            .or_else(|| self.get_instance_attribute(class, attr_name))
    }

    /// Special-case enum attribute lookups. Dispatches to the appropriate helper
    /// based on the attribute name and whether we have a known enum literal.
    ///
    /// `enum_literal` is set if we're looking this up on a known member, like `Literal[MyEnum.X]`
    ///
    /// Return None if either this is not an enum or this is not a special-case
    /// attribute.
    pub(super) fn special_case_enum_attr_lookup(
        &self,
        class: &ClassType,
        enum_literal: Option<&LitEnum>,
        metadata: &ClassMetadata,
        name: &Name,
    ) -> Option<ClassAttribute> {
        let enum_metadata = metadata.enum_metadata()?;
        if name == &VALUE {
            if !self.field_defining_class_matches(class.class_object(), &VALUE, |c| {
                c.is(KnownClass::Enum)
                    || c.is(KnownClass::EnumIntEnum)
                    || c.is(KnownClass::EnumStrEnum)
                    || c.is(KnownClass::DjangoChoices)
            }) {
                return None;
            }
            let ty = if let Some(lit_enum) = enum_literal {
                self.enum_value_lookup_on_member(class, lit_enum, enum_metadata)
            } else {
                self.enum_value_lookup_on_class(class, enum_metadata)
            };
            Some(ClassAttribute::read_write(ty))
        } else if name == &VALUE_PROP {
            if !self.field_defining_class_matches(class.class_object(), &VALUE_PROP, |c| {
                c.is(KnownClass::Enum)
                    || c.is(KnownClass::EnumIntEnum)
                    || c.is(KnownClass::EnumStrEnum)
                    || c.is(KnownClass::DjangoChoices)
            }) {
                return None;
            }
            if let Some(lit_enum) = enum_literal {
                self.get_enum_literal_or_instance_attribute(lit_enum, metadata, &VALUE)
                    .map(|attr| attr.read_only_equivalent(ReadOnlyReason::EnumMemberValue))
            } else {
                self.get_enum_or_instance_attribute(class, metadata, &VALUE)
                    .map(|attr| attr.read_only_equivalent(ReadOnlyReason::EnumMemberValue))
            }
        } else {
            None
        }
    }

    pub fn literal_typed_dict_key_name(&self, ty: &Type) -> Option<Name> {
        let Type::Literal(lit) = ty else {
            return None;
        };
        match &lit.value {
            Lit::Str(field_name) => Some(Name::new(field_name)),
            Lit::Enum(lit_enum) => {
                let metadata = self.get_metadata_for_class(lit_enum.class.class_object());
                let enum_metadata = metadata.enum_metadata()?;
                let value_ty =
                    self.enum_value_lookup_on_member(&lit_enum.class, lit_enum, enum_metadata);
                match value_ty {
                    Type::Literal(value_lit) => match value_lit.value {
                        Lit::Str(field_name) => Some(Name::new(&field_name)),
                        _ => None,
                    },
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Look up the `_value_` attribute for a specific enum member (e.g. `MyEnum.X._value_`).
    /// Whether `_value_` should be read-write is unspecified, but we need to allow assigning
    /// it in `__init__` so we make it read-write.
    fn enum_value_lookup_on_member(
        &self,
        class: &ClassType,
        lit_enum: &LitEnum,
        enum_metadata: &EnumMetadata,
    ) -> Type {
        let mixed_in = self.mixed_in_enum_data_type(class.class_object());
        let has_new = self
            .get_class_fields(class.class_object())
            .is_some_and(|f| f.contains(&dunder::NEW));
        // When `__new__` is defined, it can rewrite `_value_` at runtime, so the raw
        // RHS type is unreliable.
        // Fallbacks in order of priority: mixed-in type, type of `_value_`, `Any`
        if has_new {
            return if let Some(mixed_in) = mixed_in {
                mixed_in
            } else if let Some(value_ty) = self.type_of_enum_value(enum_metadata) {
                value_ty
            } else {
                self.heap.mk_any_implicit()
            };
        }
        let value_ty = self.enum_literal_to_value_type(lit_enum.clone(), enum_metadata.is_django);
        // Only preserve the literal type if its base class type matches the mixin exactly.
        if let Some(ref mixed_in) = mixed_in {
            let promoted = value_ty.clone().promote(self.stdlib);
            if &promoted == mixed_in {
                value_ty
            } else {
                mixed_in.clone()
            }
        } else {
            value_ty
        }
    }

    /// Prepare an enum value lookup argument for checking.
    /// Returns `Some(EnumLookupArg)` if the call is eligible for stock enum value lookup,
    /// having checked the argument and reported any diagnostic.
    pub(crate) fn prepare_enum_lookup<'b>(
        &self,
        cls: &ClassType,
        constructor_kind: &ConstructorKind,
        args: &[CallArg<'b>],
        keywords: &[CallKeyword<'b>],
        arg_errors: &ErrorCollector,
        errors: &ErrorCollector,
        context: Option<&dyn Fn() -> ErrorContext>,
    ) -> Option<EnumLookupArg> {
        if *constructor_kind == ConstructorKind::TypeOfSelf {
            return None;
        }
        let (value_arg, range, is_positional) = match (args, keywords) {
            ([CallArg::Arg(arg)], []) => (arg, arg.range(), true),
            ([], [kw]) if kw.arg.as_ref().is_some_and(|id| id.as_str() == "value") => {
                (&kw.value, kw.value.range(), false)
            }
            _ => return None,
        };
        let want = self.enum_value_lookup_target(cls)?;
        let ty = value_arg.infer(self, arg_errors);
        self.check_enum_lookup_arg(&ty, &want, range, errors, context);
        Some(EnumLookupArg {
            ty,
            range,
            is_positional,
        })
    }

    /// Restrict lookup only when primitive creation values also describe member values.
    /// Mixins and custom construction can change that relationship, so leave them unchecked.
    fn enum_value_lookup_target(&self, cls: &ClassType) -> Option<Type> {
        let metadata = self.get_metadata_for_class(cls.class_object());
        let enum_metadata = metadata.enum_metadata()?;

        if let Some(custom_metaclass) = metadata.custom_metaclass()
            && custom_metaclass.class_object() != self.stdlib.enum_meta().class_object()
        {
            return None;
        }
        if self
            .get_base_types_for_class(cls.class_object())
            .iter()
            .any(|base| {
                !self.has_superclass(base.class_object(), self.stdlib.enum_class().class_object())
            })
        {
            return None;
        }
        for method in [&Name::new_static("_missing_"), &dunder::NEW, &dunder::INIT] {
            if let Some(field) =
                self.get_class_member_with_defining_class(cls.class_object(), method)
                && !field.defining_class.is(KnownClass::Enum)
                && !field.defining_class.is(KnownClass::Object)
            {
                return None;
            }
        }

        let members = self.get_enum_members(cls.class_object());
        if members.is_empty() {
            return None;
        }
        let mut domains = members
            .into_iter()
            .map(|lit| {
                let Lit::Enum(lit_enum) = lit else {
                    unreachable!("enum members must be represented as enum literals");
                };
                let value = self.enum_literal_to_value_type(*lit_enum, enum_metadata.is_django);
                self.primitive_enum_lookup_domain(&value)
            })
            .collect::<Option<Vec<_>>>()?;
        domains.push(self.heap.mk_class_type(cls.clone()));
        Some(self.unions(domains))
    }

    /// Known primitive equality domains, widening explicit literals only for lookup.
    /// An unknown value alternative makes the entire member domain unreliable.
    fn primitive_enum_lookup_domain(&self, ty: &Type) -> Option<Type> {
        let domains = ty
            .clone()
            .into_unions()
            .into_iter()
            .map(|value| {
                let cls = match &value {
                    Type::Literal(lit) => lit.value.general_class_type(self.stdlib),
                    Type::LiteralString(_) => self.stdlib.str(),
                    Type::ClassType(cls) => cls,
                    _ => return None,
                };
                let promoted = self.heap.mk_class_type(cls.clone());
                if cls.is(KnownClass::Int)
                    || cls.is(KnownClass::Float)
                    || cls.is(KnownClass::Complex)
                    || cls.is(KnownClass::Bool)
                {
                    Some(self.unions(vec![
                        promoted,
                        self.heap.mk_class_type(self.stdlib.int().clone()),
                        self.heap.mk_class_type(self.stdlib.float().clone()),
                        self.heap.mk_class_type(self.stdlib.complex().clone()),
                    ]))
                } else if cls.is(KnownClass::Str) {
                    Some(promoted)
                } else if cls.is(KnownClass::Bytes) {
                    Some(self.unions(vec![
                        promoted,
                        self.heap.mk_class_type(self.stdlib.bytearray().clone()),
                    ]))
                } else {
                    None
                }
            })
            .collect::<Option<Vec<_>>>()?;
        Some(self.unions(domains))
    }

    /// Report only known mismatches: validation inputs may have a compatible union branch,
    /// and unmodeled types may compare equal through custom equality. Intersection narrowings
    /// still retain each concrete restriction rather than accepting a broad TypeVar bound alone.
    fn is_enum_lookup_compatible(&self, got: &Type, want: &Type) -> bool {
        if self.is_subset_eq(got, want) {
            return true;
        }
        match got {
            Type::Quantified(q) if q.is_type_var() => {
                let bound = q.upper_bound(self.stdlib, self.heap);
                self.is_enum_lookup_compatible(&bound, want)
            }
            Type::TypeVar(tv) => {
                let bound = tv.upper_bound(self.stdlib, self.heap);
                self.is_enum_lookup_compatible(&bound, want)
            }
            Type::Union(u) => u
                .members
                .iter()
                .any(|t| self.is_enum_lookup_compatible(t, want)),
            Type::Intersect(inner) => {
                let (tys, _) = inner.as_ref();
                tys.iter().all(|t| self.is_enum_lookup_compatible(t, want))
            }
            Type::None => false,
            _ => self.primitive_enum_lookup_domain(got).is_none(),
        }
    }

    fn check_enum_lookup_arg(
        &self,
        got: &Type,
        want: &Type,
        range: TextRange,
        errors: &ErrorCollector,
        context: Option<&dyn Fn() -> ErrorContext>,
    ) {
        if self.is_enum_lookup_compatible(got, want) {
            return;
        }
        self.check_type_as_call_argument(got, want, range, errors, &|| {
            let mut tcc = TypeCheckContext::of_kind(TypeCheckKind::CallArgument(
                Some(Name::new_static("value")),
                None,
            ));
            if let Some(ctx) = context {
                tcc = tcc.with_context(Some(ctx()));
            }
            tcc
        });
    }

    /// Look up the `_value_` attribute for an enum type (not a specific member).
    /// Whether `_value_` should be read-write is unspecified, but we need to allow assigning
    /// it in `__init__` so we make it read-write.
    fn enum_value_lookup_on_class(&self, class: &ClassType, enum_metadata: &EnumMetadata) -> Type {
        let mixed_in = self.mixed_in_enum_data_type(class.class_object());
        let has_new = self
            .get_class_fields(class.class_object())
            .is_some_and(|f| f.contains(&dunder::NEW));
        // When `__new__` is defined, it can rewrite `_value_` at runtime. Fall back to the
        // mixed-in type, or `Any` if there is no mixin.
        if has_new {
            return if let Some(mixed_in) = mixed_in {
                mixed_in
            } else {
                self.heap.mk_any_implicit()
            };
        }
        // The `_value_` annotation on `enum.Enum` is `Any`; we can infer a better type.
        let enum_value_types: Vec<_> = self
            .get_enum_members(class.class_object())
            .into_iter()
            .filter_map(|lit| {
                let Lit::Enum(lit_enum) = lit else {
                    return None;
                };
                let value_ty = self.enum_literal_to_value_type(*lit_enum, enum_metadata.is_django);
                Some(if let Some(ref mixed_in) = mixed_in {
                    let promoted = value_ty.clone().promote(self.stdlib);
                    if &promoted == mixed_in {
                        value_ty
                    } else {
                        mixed_in.clone()
                    }
                } else {
                    value_ty
                })
            })
            .collect();
        if enum_value_types.is_empty() {
            // Don't assume Never if there are no members, because they may be created
            // dynamically and we don't want false-positives downstream.
            if let Some(mixed_in) = mixed_in {
                mixed_in
            } else {
                self.heap.mk_any_implicit()
            }
        } else {
            self.unions(enum_value_types)
        }
    }

    /// If this enum mixes in a data type by inheriting from it, return the mixed-in type.
    /// Searches all bases, not just the first, to handle cases like
    /// `IntegerChoices(Choices, IntEnum)` where the data type comes from `IntEnum`.
    /// A non-enum base is only treated as a data type if it has `__new__`
    /// inherited from a class other than `object`, which is how Python
    /// distinguishes data type mixins (`int`, `str`, `float`, and their
    /// subclasses like `MyStr(str)`) from regular method mixins.
    fn mixed_in_enum_data_type(&self, class: &Class) -> Option<Type> {
        let bases = self.get_base_types_for_class(class);
        let enum_class = self.stdlib.enum_class();
        for base in bases.iter() {
            if *base == *enum_class {
                continue;
            } else if self.has_superclass(base.class_object(), enum_class.class_object()) {
                if let Some(ty) = self.mixed_in_enum_data_type(base.class_object()) {
                    return Some(ty);
                }
            } else {
                let is_data_type = self
                    .get_class_member_with_defining_class(base.class_object(), &dunder::NEW)
                    .is_some_and(|field| !field.defining_class.is(KnownClass::Object));
                if is_data_type {
                    return Some(self.heap.mk_class_type(base.clone()));
                }
            }
        }
        None
    }

    /// Convert an enum literal's raw value type to its `.value` type.
    fn enum_literal_to_value_type(&self, lit_enum: LitEnum, is_django: bool) -> Type {
        let ty = if is_django {
            transform_django_enum_value(lit_enum.ty, self.heap)
        } else {
            lit_enum.ty
        };
        let auto_ty = self.auto_value_type(lit_enum.class.class_object());
        ty.transform(&mut |t| {
            if matches!(t, Type::ClassType(cls) if cls.is(KnownClass::EnumAuto)) {
                *t = auto_ty.clone();
            }
        })
    }

    /// Determine the type that `auto()` produces for the given enum class.
    /// 1. If a data type is mixed in (e.g. `class E(str, Enum)`), `auto()` produces
    ///    that type because `__new__` converts the raw value.
    /// 2. Otherwise, look up `_generate_next_value_` on the class, defaulting to `int` for `enum.Enum`
    fn auto_value_type(&self, cls: &Class) -> Type {
        // A mixed-in data type takes priority: the enum's `__new__` converts values to that type.
        if let Some(mixed_in) = self.mixed_in_enum_data_type(cls) {
            return mixed_in;
        }
        self.get_class_member_with_defining_class(cls, &GENERATE_NEXT_VALUE)
            .and_then(|field| {
                // `enum.Enum` declares an `Any` return, but at runtime it generates `int`
                if field.defining_class.is(KnownClass::Enum) {
                    Some(self.heap.mk_class_type(self.stdlib.int().clone()))
                } else {
                    field.value.ty().callable_return_type(self.heap)
                }
            })
            .unwrap_or_else(|| self.heap.mk_any_implicit()) // Fall back to `Any` if `_generate_next_value_` is missing or not callable
    }

    /// Whether the class can be represented by its declared enum member literals.
    /// Flag-derived enums are excluded because they allow additional combined values.
    /// See <https://typing.python.org/en/latest/spec/enums.html#enum-literal-expansion>.
    pub(crate) fn is_expandable_enum(&self, cls: &Class) -> bool {
        self.get_metadata_for_class(cls).is_enum()
            && !self.has_superclass(cls, self.stdlib.enum_flag().class_object())
    }

    /// Whether the named member's literal is equivalent to the entire enum type.
    /// `member` is the canonical member name, after resolving aliases.
    pub fn enum_literal_covers_class(&self, cls: &Class, member: &Name) -> bool {
        if !self.is_expandable_enum(cls) {
            return false;
        }
        let Some(fields) = self.get_class_fields(cls) else {
            return false;
        };
        let mut members = fields
            .names()
            .filter_map(|name| self.get_enum_member(cls, name))
            .peekable();
        // Stop at the first different member, without constructing the full member set.
        members.peek().is_some()
            && members.all(|literal| {
                let Lit::Enum(enum_lit) = literal else {
                    unreachable!("enum members must be represented as enum literals");
                };
                enum_lit.member == *member
            })
    }

    /// Count distinct members only for enums eligible for literal expansion.
    pub fn get_expandable_enum_member_count(&self, cls: &Class) -> Option<usize> {
        if self.is_expandable_enum(cls) {
            Some(self.get_enum_members(cls).len())
        } else {
            None
        }
    }

    /// Enum handling:
    /// - Check whether the field is a member (which depends only on its type and name)
    /// - Validate that a member should not have an annotation, and should respect any explicit annotation on `_value_`
    ///
    /// We currently skip the check for `_value_` if the class defines `__new__`, since that can
    /// change the value of the enum member. https://docs.python.org/3/howto/enum.html#when-to-use-new-vs-init
    pub fn get_enum_class_field_type(
        &self,
        class: &Class,
        name: &Name,
        direct_annotation: Option<&Annotation>,
        ty: &Type,
        field_definition: &ClassFieldDefinition,
        is_descriptor: bool,
        range: TextRange,
        errors: &ErrorCollector,
    ) -> Option<Type> {
        if is_descriptor {
            return None;
        }
        // Extract alias_of from field_definition for enum alias detection
        let alias_of = match field_definition {
            ClassFieldDefinition::AssignedInBody { alias_of, .. } => alias_of.as_ref(),
            _ => None,
        };
        let metadata = self.get_metadata_for_class(class);
        if let Some(enum_) = metadata.enum_metadata()
            && self.is_valid_enum_member(name, ty, field_definition)
        {
            if direct_annotation.is_some() {
                self.error(
                    errors, range,ErrorKind::InvalidAnnotation,
                    format!("Enum member `{name}` may not be annotated directly. Instead, annotate the `_value_` attribute."),
                );
            }
            if enum_.has_value
                && let Some(enum_value_ty) = self.type_of_enum_value(enum_)
                && !self
                    .get_class_fields(class)
                    .is_some_and(|f| f.contains(&dunder::NEW))
                && (!ty.is_ellipsis_value() || !self.module().path().is_interface())
            {
                self.check_enum_value_annotation(ty, &enum_value_ty, name, range, errors);
            }
            // If this field is an alias (value is a simple name referring to another field),
            // look up the aliased member and return its type instead of creating a new enum literal.
            if let Some(aliased_name) = alias_of
                && let Some(aliased_member_lit) = self.get_enum_member(class, aliased_name)
            {
                return Some(aliased_member_lit.to_implicit_type());
            }
            // `member(x)` makes `x` a member, so the member's value is `x`, the same way
            // `nonmember(x)` below is just `x`.
            let value_ty = if let Type::ClassType(cls) = ty
                && cls.is(KnownClass::EnumMember)
                && let [targ] = cls.targs().as_slice()
            {
                targ
            } else {
                ty
            };
            Some(
                Lit::Enum(Box::new(LitEnum {
                    class: enum_.cls.clone(),
                    member: name.clone(),
                    ty: self.solver().force(value_ty.clone()),
                }))
                .to_implicit_type(),
            )
        } else if let Type::ClassType(cls) = &ty
            && cls.is(KnownClass::EnumNonmember)
            && let [targ] = cls.targs().as_slice()
        {
            Some(targ.clone())
        } else {
            None
        }
    }

    /// Look up the `_value_` attribute of an enum class. This field has to be a plain instance
    /// attribute annotated in the class body; it is used to validate enum member values, which are
    /// supposed to all share this type.
    ///
    /// TODO(stroxler): We don't currently enforce in this function that it is
    /// an instance attribute annotated in the class body. Should we? It is unclear; this helper
    /// is only used to validate enum members, not to produce errors on invalid `_value_`
    fn type_of_enum_value(&self, enum_: &EnumMetadata) -> Option<Type> {
        let field = self.get_class_member(enum_.cls.class_object(), &VALUE)?;
        if field.is_simple_instance_attribute() {
            Some(field.ty())
        } else {
            None
        }
    }

    fn check_enum_value_annotation(
        &self,
        mut value: &Type,
        annotation: &Type,
        member: &Name,
        range: TextRange,
        errors: &ErrorCollector,
    ) {
        if matches!(value, Type::Tuple(_)) {
            // TODO: check tuple values against constructor signature
            // see https://typing.python.org/en/latest/spec/enums.html#member-values
            return;
        }
        if matches!(value, Type::ClassType(cls) if cls.is(KnownClass::EnumAuto)) {
            return;
        }
        if let Type::ClassType(cls) = value
            && cls.is(KnownClass::EnumMember)
            && let [member_targ] = cls.targs().as_slice()
        {
            value = member_targ;
        }
        if !self.is_subset_eq(value, annotation) {
            self.error(
                errors, range, ErrorKind::BadAssignment,
                format!(
                    "Enum member `{member}` has type `{}`, must match the `_value_` attribute annotation of `{}`",
                    self.for_display(value.clone()),
                    self.for_display(annotation.clone()),
                ),
            );
        }
    }
}
