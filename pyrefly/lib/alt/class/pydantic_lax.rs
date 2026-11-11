/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use pyrefly_python::module_name::ModuleName;
use pyrefly_types::class::ClassType;
use pyrefly_types::identity::IdentityIgnored;
use pyrefly_types::keywords::ConverterMap;
use pyrefly_types::known_class::KnownClass;
use pyrefly_types::literal::Lit;
use pyrefly_types::quantified::Quantified;
use pyrefly_types::tuple::Tuple;
use ruff_python_ast::name::Name;
use starlark_map::ordered_map::OrderedMap;

use crate::alt::answers::LookupAnswer;
use crate::alt::answers_solver::AnswersSolver;
use crate::alt::types::class_metadata::DataclassKind;
use crate::binding::pydantic::GET_PYDANTIC_CORE_SCHEMA;
use crate::module::typeshed::typeshed;
use crate::types::class::Class;
use crate::types::types::Type;

const LAX_PREFIX: &str = "Lax";

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

impl<'ctx, 'answer, Ans: LookupAnswer> AnswersSolver<'ctx, 'answer, Ans> {
    fn lax_display_name_for_class(&self, cls: &ClassType) -> Name {
        Name::new(format!(
            "{}{}",
            LAX_PREFIX,
            capitalize_first(cls.name().as_str())
        ))
    }

    fn types_to_lax_union(&self, base_type: &ClassType, types: &[&ClassType]) -> Type {
        let display_name = self.lax_display_name_for_class(base_type);
        let expanded_types: Vec<Type> = types
            .iter()
            .map(|cls| self.heap.mk_class_type((*cls).clone()))
            .collect();
        let mut union_type = self.unions(expanded_types);
        if let Type::Union(ref mut boxed_union) = union_type {
            boxed_union.display_name = IdentityIgnored(Some((ModuleName::unknown(), display_name)));
        }
        union_type
    }

    fn expand_types(&self, types: &[Type]) -> Vec<Type> {
        types
            .iter()
            .map(|t| self.expand_type_for_lax_mode(t))
            .collect()
    }

    fn get_atomic_lax_conversion(&self, ty: &Type) -> Option<Type> {
        match ty {
            Type::ClassType(cls) if cls == self.stdlib.bool() => Some(self.types_to_lax_union(
                self.stdlib.bool(),
                &[
                    self.stdlib.bool(),
                    self.stdlib.int(),
                    self.stdlib.float(),
                    self.stdlib.str(),
                    self.stdlib.decimal(),
                ],
            )),
            Type::ClassType(cls) if cls == self.stdlib.int() || cls == self.stdlib.float() => {
                Some(self.types_to_lax_union(
                    cls,
                    &[
                        self.stdlib.int(),
                        self.stdlib.float(),
                        self.stdlib.bool(),
                        self.stdlib.str(),
                        self.stdlib.bytes(),
                        self.stdlib.decimal(),
                    ],
                ))
            }
            Type::ClassType(cls) if cls == self.stdlib.bytes() || cls == self.stdlib.str() => {
                Some(self.types_to_lax_union(
                    cls,
                    &[
                        self.stdlib.bytes(),
                        self.stdlib.str(),
                        self.stdlib.bytearray(),
                    ],
                ))
            }
            Type::ClassType(cls) if cls == self.stdlib.date() || cls == self.stdlib.datetime() => {
                Some(self.types_to_lax_union(
                    cls,
                    &[
                        self.stdlib.date(),
                        self.stdlib.datetime(),
                        self.stdlib.int(),
                        self.stdlib.float(),
                        self.stdlib.str(),
                        self.stdlib.bytes(),
                        self.stdlib.decimal(),
                    ],
                ))
            }
            Type::ClassType(cls) if cls == self.stdlib.time() || cls == self.stdlib.timedelta() => {
                Some(self.types_to_lax_union(
                    cls,
                    &[
                        cls,
                        self.stdlib.int(),
                        self.stdlib.float(),
                        self.stdlib.str(),
                        self.stdlib.bytes(),
                        self.stdlib.decimal(),
                    ],
                ))
            }
            Type::ClassType(cls) if cls == self.stdlib.decimal() => Some(self.types_to_lax_union(
                self.stdlib.decimal(),
                &[
                    self.stdlib.decimal(),
                    self.stdlib.int(),
                    self.stdlib.float(),
                    self.stdlib.str(),
                ],
            )),
            Type::ClassType(cls) if cls == self.stdlib.path() || cls == self.stdlib.uuid() => {
                Some(self.types_to_lax_union(cls, &[cls, self.stdlib.str()]))
            }
            _ => None,
        }
    }

    /// Check each union alternative with its lax conversion and refine bool's literal inputs.
    pub fn pydantic_lax_accepts_literal(&self, expected: &Type, literal: &Type) -> bool {
        if let Type::Union(union) = expected {
            return union
                .members
                .iter()
                .any(|member| self.pydantic_lax_accepts_literal(member, literal));
        }
        if let Type::ClassType(cls) = expected
            && cls == self.stdlib.bool()
            && let Type::Literal(lit) = literal
        {
            match &lit.value {
                Lit::Int(value) => return matches!(value.as_i64(), Some(0 | 1)),
                Lit::Str(value) => {
                    return matches!(
                        value.to_ascii_lowercase().as_str(),
                        "0" | "1"
                            | "f"
                            | "false"
                            | "n"
                            | "no"
                            | "off"
                            | "on"
                            | "t"
                            | "true"
                            | "y"
                            | "yes"
                    );
                }
                _ => {}
            }
        }
        self.is_subset_eq(literal, &self.expand_type_for_lax_mode(expected))
    }

    fn get_container_lax_conversion(
        &self,
        class_obj: &Class,
        expanded_targs: &[Type],
    ) -> Option<Type> {
        let first_ty = expanded_targs
            .first()
            .cloned()
            .unwrap_or_else(|| self.heap.mk_any_implicit());
        // All single-element containers use Iterable to handle invariance issues
        // This allows passing any iterable type (list, set, deque, frozenset, etc.)
        // Note: dict is handled separately in expand_type_for_lax_mode to avoid expanding the key type
        if class_obj == self.stdlib.list_object()
            || class_obj == self.stdlib.set_object()
            || class_obj == self.stdlib.frozenset_object()
            || class_obj.is(KnownClass::CollectionsDeque)
            || class_obj.is(KnownClass::TypingSequence)
            || class_obj.is(KnownClass::TypingIterable)
        {
            return Some(self.heap.mk_class_type(self.stdlib.iterable(first_ty)));
        }
        None
    }

    fn get_regex_pattern_lax_conversion(
        &self,
        class_obj: &Class,
        ty: &Type,
        targs: &[Type],
    ) -> Option<Type> {
        if class_obj.is(KnownClass::RePattern) || class_obj.is(KnownClass::TypingPattern) {
            let pattern_input = targs
                .first()
                .cloned()
                .unwrap_or_else(|| self.heap.mk_any_implicit());
            return Some(self.union(ty.clone(), pattern_input));
        }
        None
    }

    fn get_tuple_element_type(&self, tuple: &Tuple) -> Type {
        match tuple {
            Tuple::Unbounded(elem) => self.expand_type_for_lax_mode(elem),
            Tuple::Concrete(elems) => {
                let expanded_elems = self.expand_types(elems);
                self.unions(expanded_elems)
            }
            // this case is not a valid pydantic case
            Tuple::Unpacked(_) => self.heap.mk_any_explicit(),
        }
    }

    pub fn expand_type_for_lax_mode(&self, ty: &Type) -> Type {
        match ty {
            Type::None => ty.clone(),
            // Literal types have no lax coercion - they require exact values
            Type::Literal(_) => ty.clone(),
            Type::LiteralString(_) => ty.clone(),
            Type::Type(inner) => self.heap.mk_type_of(self.expand_type_for_lax_mode(inner)),
            // Tuple types: convert to Iterable[T] where T is a union of expanded element types
            Type::Tuple(tuple) => self
                .heap
                .mk_class_type(self.stdlib.iterable(self.get_tuple_element_type(tuple))),
            Type::ClassType(cls) => {
                if let Some(converted) = self.get_atomic_lax_conversion(ty) {
                    return converted;
                }
                let class_obj = cls.class_object();
                let metadata = self.get_metadata_for_class(class_obj);
                // BaseModel's built-in hook preserves model validation. Other hooks, model
                // validators that replace the raw input, and `from_attributes` may accept any
                // input.
                if self
                    .get_non_synthesized_class_member_and_defining_class(
                        class_obj,
                        &GET_PYDANTIC_CORE_SCHEMA,
                    )
                    .is_some_and(|member| !member.defining_class.is(KnownClass::PydanticBaseModel))
                    || metadata.dataclass_metadata().is_some_and(|dataclass| {
                        dataclass.pydantic_validator_replaces_all_input()
                            || dataclass
                                .init_defaults
                                .pydantic_validation_flags
                                .may_validate_from_attributes()
                    })
                {
                    return self.heap.mk_any_explicit();
                }
                // Pydantic validates a `NewType` with the schema of its base type.
                if metadata.is_new_type()
                    && let Some(base) = self.get_base_types_for_class(class_obj).iter().next()
                {
                    return self.expand_type_for_lax_mode(&self.heap.mk_class_type(base.clone()));
                }
                if metadata.is_pydantic_model()
                    || metadata.dataclass_metadata().is_some_and(|dataclass| {
                        matches!(
                            dataclass.kind,
                            DataclassKind::Dataclass {
                                is_stdlib_dataclass: true,
                                ..
                            }
                        )
                    })
                {
                    let mapping = self.heap.mk_class_type(self.stdlib.mapping(
                        self.heap.mk_class_type(self.stdlib.str().clone()),
                        self.heap.mk_any_explicit(),
                    ));
                    return self.union(ty.clone(), mapping);
                }
                let targs = cls.targs().as_slice();

                // Special handling for dict and Mapping: don't expand key type
                // (Mapping is invariant in key, so expanding it would make
                // dict[str, V] not assignable to Mapping[LaxStr, V])
                if class_obj == self.stdlib.dict_object()
                    || class_obj == self.stdlib.mapping_object()
                {
                    let key_ty = targs
                        .first()
                        .cloned()
                        .unwrap_or_else(|| self.heap.mk_any_implicit());
                    let val_ty = targs
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| self.heap.mk_any_implicit());
                    let expanded_val = self.expand_type_for_lax_mode(&val_ty);
                    return self
                        .heap
                        .mk_class_type(self.stdlib.mapping(key_ty, expanded_val));
                }

                if let Some(converted) = self.get_regex_pattern_lax_conversion(class_obj, ty, targs)
                {
                    return converted;
                }

                let expanded_targs = self.expand_types(targs);

                // Check for container type conversions
                if let Some(converted) =
                    self.get_container_lax_conversion(class_obj, &expanded_targs)
                {
                    return converted;
                }

                // Enums and namedtuples have built-in schemas that accept more than instances,
                // as do some stdlib classes whose lax conversions we don't model precisely, so
                // we map all stdlib classes to `Any` as a conservative approximation. Pydantic
                // validates other classes with `isinstance`.
                if metadata.is_enum()
                    || metadata.named_tuple_metadata().is_some()
                    || typeshed().is_ok_and(|ts| ts.has_module(class_obj.module_name()))
                {
                    return self.heap.mk_any_explicit();
                }
                let mut instance_ty = cls.clone();
                for (param, targ) in instance_ty.targs_mut().iter_paired_mut() {
                    *targ = Quantified::as_gradual_type_helper(param.kind(), None);
                }
                self.heap.mk_class_type(instance_ty)
            }
            Type::Union(f) => {
                let expanded_members = self.expand_types(&f.members);
                self.unions(expanded_members)
            }
            // Runtime validation determines whether unsupported forms such as callables are valid.
            _ => self.heap.mk_any_explicit(),
        }
    }

    pub fn build_pydantic_lax_conversion_table(&self, field_types: &[Type]) -> ConverterMap {
        let mut table = OrderedMap::new();

        for field_ty in field_types {
            if table.contains_key(field_ty) {
                continue;
            }

            let expanded = self.expand_type_for_lax_mode(field_ty);
            table.insert(field_ty.clone(), expanded);
        }

        ConverterMap::from_map(table)
    }
}
