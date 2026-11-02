/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::collections::HashSet;
use std::iter::once;

use dupe::Dupe;
use lsp_types::CodeActionKind;
use pyrefly_build::handle::Handle;
use pyrefly_python::docstring::dedent_block_preserving_layout;
use pyrefly_python::module::Module;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::short_identifier::ShortIdentifier;
use pyrefly_python::sys_info::PythonVersion;
use pyrefly_types::callable::Callable;
use pyrefly_types::callable::DefaultValue;
use pyrefly_types::callable::Param;
use pyrefly_types::callable::ParamList;
use pyrefly_types::callable::Params;
use pyrefly_types::callable::PrefixParam;
use pyrefly_types::callable::Required;
use pyrefly_types::literal::Lit;
use pyrefly_types::quantified::QuantifiedOrigin;
use pyrefly_types::stdlib::Stdlib;
use pyrefly_types::type_output::AnnotationPart;
use pyrefly_types::type_var::Restriction;
use pyrefly_types::types::TParams;
use pyrefly_types::types::Type;
use ruff_python_ast::Stmt;
use ruff_python_ast::StmtClassDef;
use ruff_python_ast::name::Name;
use ruff_text_size::Ranged;
use ruff_text_size::TextRange;
use ruff_text_size::TextSize;
use starlark_map::Hashed;

use super::extract_shared::DEFAULT_INDENT;
use super::extract_shared::line_indent_and_start;
use super::extract_shared::prepare_insertion_text;
use super::extract_shared::selection_anchor;
use super::extract_shared::target_insertion_point;
use super::extract_shared::unique_name;
use crate::binding::binding::KeyClass;
use crate::state::annotation_renderer::AnnotationRenderer;
use crate::state::lsp::ImportFormat;
use crate::state::lsp::LocalRefactorCodeAction;
use crate::state::lsp::Transaction;

struct AbstractMemberInfo {
    name: Name,
    ty: Type,
    defining_module: Module,
    docstring_range: Option<TextRange>,
    is_class_var: bool,
    is_coroutine: bool,
}

/// Builds a quick fix that implements all inherited abstract members in a class.
pub(crate) fn implement_abstract_members_code_actions(
    transaction: &Transaction<'_>,
    handle: &Handle,
    selection: TextRange,
    import_format: ImportFormat,
) -> Option<Vec<LocalRefactorCodeAction>> {
    let module_info = transaction.get_module_info(handle)?;
    let ast = transaction.get_ast(handle)?;
    let source = module_info.contents();
    let selection_point = selection_anchor(source, selection);
    let class_def = find_class_header(&ast.body, selection_point)?;
    let (class_indent, _) = line_indent_and_start(source, class_def.start())?;
    // A suite on the header line, as in `class C(Base): pass`, has no line to insert into.
    let first = class_def.body.first()?;
    let (_, body_line_start) = line_indent_and_start(source, first.start())?;
    if !source[body_line_start.to_usize()..first.start().to_usize()]
        .trim()
        .is_empty()
    {
        return None;
    }
    let (indent, insert_range, replaces_pass) = target_insertion_point(class_def, source)?;
    let indent_unit = indent
        .strip_prefix(&class_indent)
        .filter(|unit| !unit.is_empty())
        .unwrap_or(DEFAULT_INDENT);

    let mut members = transaction
        .ad_hoc_solve(
            handle,
            "implement_abstract_members",
            |solver| -> Option<Vec<AbstractMemberInfo>> {
                let key = KeyClass(ShortIdentifier::new(&class_def.name));
                let class_idx = solver.bindings().key_to_idx_hashed_opt(Hashed::new(&key))?;
                let class = solver.get_idx(class_idx).0.clone()?;
                // A protocol's members are its interface, so they stay unimplemented.
                if solver.get_metadata_for_class(&class).is_protocol() {
                    return None;
                }
                let abstract_members = solver.get_abstract_members_for_class(&class);
                let mut infos = Vec::new();
                for name in abstract_members.unimplemented_abstract_methods() {
                    let member = solver.get_class_member_with_defining_class(&class, name)?;
                    if member.defining_class == class {
                        continue;
                    }
                    let mut ty = member.value.ty();
                    // An `async def` without `yield` returns a coroutine. An async generator
                    // returns its async iterator directly, so it is rendered as a plain `def`,
                    // which keeps its call contract without needing a `yield`.
                    let mut is_coroutine = false;
                    if ty
                        .toplevel_func_metadata()
                        .is_some_and(|m| m.flags.is_async)
                    {
                        ty.transform_toplevel_callable_signatures(|callable, _| {
                            if let Some((_, _, ret)) = solver.unwrap_coroutine(&callable.ret) {
                                callable.ret = ret;
                                is_coroutine = true;
                            }
                        });
                    }
                    infos.push(AbstractMemberInfo {
                        name: name.clone(),
                        ty,
                        is_coroutine,
                        is_class_var: member.value.is_class_var(),
                        defining_module: member.defining_class.module().dupe(),
                        docstring_range: solver
                            .get_class_fields(&member.defining_class)
                            .and_then(|fields| fields.field_docstring_range(name)),
                    });
                }
                Some(infos)
            },
        )
        .flatten()?;

    if members.is_empty() {
        return None;
    }

    members.sort_by(|left, right| left.name.cmp(&right.name));
    let stdlib = transaction.get_stdlib(handle);
    let mut renderer = MemberRenderer {
        stdlib: &stdlib,
        python_version: handle.sys_info().version(),
        parts: Vec::new(),
    };
    if !replaces_pass {
        // A blank line separates the stubs from the existing body.
        renderer.text(&prepare_insertion_text(source, insert_range.start(), "\n"));
    }
    for (i, member) in members.iter().enumerate() {
        if i > 0 {
            renderer.text("\n\n");
        }
        renderer.member(member, &indent, indent_unit)?;
    }
    renderer.text("\n");
    let annotations = AnnotationRenderer::new(transaction, handle, ast, &stdlib, import_format);
    let rendered = annotations.render_parts(&renderer.parts)?;
    let imports = annotations.import_edits(&rendered);
    let edits = once((module_info.dupe(), insert_range, rendered.text))
        .chain(
            imports
                .into_iter()
                .map(|(range, text)| (module_info.dupe(), range, text)),
        )
        .collect();

    Some(vec![LocalRefactorCodeAction {
        title: "Implement abstract members".to_owned(),
        edits,
        kind: CodeActionKind::QuickFix,
    }])
}

/// Finds the class whose header, from its first decorator through its bases, contains
/// `position`. Offering the action only there keeps it out of automatic code action
/// requests elsewhere in a class body.
fn find_class_header(body: &[Stmt], position: TextSize) -> Option<&StmtClassDef> {
    body.iter().find_map(|stmt| {
        let Stmt::ClassDef(class_def) = stmt else {
            return None;
        };
        if !class_def.range().contains_inclusive(position) {
            return None;
        }
        let header_end = class_def
            .arguments
            .as_ref()
            .map_or(class_def.name.end(), |arguments| arguments.end());
        if position <= header_end {
            Some(class_def)
        } else {
            find_class_header(&class_def.body, position)
        }
    })
}

/// Builds source text while retaining the module references used by annotations.
struct MemberRenderer<'a> {
    stdlib: &'a Stdlib,
    python_version: PythonVersion,
    parts: Vec<AnnotationPart>,
}

impl MemberRenderer<'_> {
    fn text(&mut self, text: &str) {
        if let Some(AnnotationPart::Text(previous)) = self.parts.last_mut() {
            previous.push_str(text);
        } else {
            self.parts.push(AnnotationPart::Text(text.to_owned()));
        }
    }

    fn reference(&mut self, module: &str, name: &str) {
        self.parts.push(AnnotationPart::Reference {
            module: ModuleName::from_str(module),
            name: name.to_owned(),
        });
    }

    fn annotation(&mut self, ty: &Type) {
        self.parts
            .extend(ty.get_annotation_parts(Some(self.stdlib)));
    }

    fn member(
        &mut self,
        member: &AbstractMemberInfo,
        indent: &str,
        indent_unit: &str,
    ) -> Option<()> {
        let Some(metadata) = member
            .ty
            .toplevel_func_metadata()
            .filter(|_| !member.is_class_var)
        else {
            self.text(indent);
            self.text(member.name.as_str());
            self.text(": ");
            if member.is_class_var {
                self.reference("typing", "ClassVar");
                self.text("[");
            }
            self.annotation(&member.ty);
            if member.is_class_var {
                self.text("]");
            }
            return Some(());
        };
        let property = member.ty.property_metadata();
        let getter = property.map(|p| &p.getter);
        let setter = property.and_then(|p| p.setter.as_ref());
        let is_static = metadata.flags.is_staticmethod;
        let is_class = metadata.flags.is_classmethod;
        let receiver_decorator = if is_class {
            Some("classmethod")
        } else if is_static {
            Some("staticmethod")
        } else {
            None
        };
        let overloaded = member.ty.toplevel_callable_signatures().count() > 1;
        for (index, (callable, tparams)) in getter
            .unwrap_or(&member.ty)
            .toplevel_callable_signatures()
            .enumerate()
        {
            if index > 0 {
                self.text("\n\n");
            }
            if overloaded {
                self.text(&format!("{indent}@"));
                self.reference("typing", "overload");
                self.text("\n");
            }
            if metadata.flags.is_cached_property {
                self.text(&format!("{indent}@"));
                self.reference("functools", "cached_property");
                self.text("\n");
                self.def_line(member, indent, None);
            } else if getter.is_some() {
                self.def_line(member, indent, Some("property"));
            } else {
                self.def_line(member, indent, receiver_decorator);
            }
            self.signature(callable, tparams.map(|p| p.as_ref()), !is_static)?;
            self.text(":\n");
            if overloaded {
                self.text(&format!("{indent}{indent_unit}..."));
            } else {
                self.body(member, indent, indent_unit);
            }
        }
        if overloaded {
            // The overloads define the contract. The implementation accepts every overload.
            self.text("\n\n");
            self.def_line(member, indent, receiver_decorator);
            self.text(if is_class {
                "(cls, *args, **kwargs) -> "
            } else if is_static {
                "(*args, **kwargs) -> "
            } else {
                "(self, *args, **kwargs) -> "
            });
            self.reference("typing", "Any");
            self.text(":\n");
            self.body(member, indent, indent_unit);
        }
        if let Some(setter) = setter {
            let (callable, tparams) = setter.toplevel_callable_signatures().next()?;
            self.text("\n\n");
            self.def_line(member, indent, Some(&format!("{}.setter", member.name)));
            self.signature(callable, tparams.map(|p| p.as_ref()), true)?;
            self.text(&format!(
                ":\n{indent}{indent_unit}raise NotImplementedError"
            ));
        }
        Some(())
    }

    /// Writes an optional decorator line, then `def` or `async def` and the member's name.
    fn def_line(&mut self, member: &AbstractMemberInfo, indent: &str, decorator: Option<&str>) {
        if let Some(decorator) = decorator {
            self.text(&format!("{indent}@{decorator}\n"));
        }
        let keyword = if member.is_coroutine {
            "async def"
        } else {
            "def"
        };
        self.text(&format!("{indent}{keyword} {}", member.name));
    }

    fn body(&mut self, member: &AbstractMemberInfo, indent: &str, indent_unit: &str) {
        let body_indent = format!("{indent}{indent_unit}");
        if let Some(range) = member.docstring_range
            && let Some(docstring) =
                dedent_block_preserving_layout(member.defining_module.code_at(range))
        {
            for line in docstring.trim_end().lines() {
                self.text(&body_indent);
                self.text(line);
                self.text("\n");
            }
        }
        self.text(&body_indent);
        self.text("raise NotImplementedError");
    }

    fn signature(
        &mut self,
        callable: &Callable,
        tparams: Option<&TParams>,
        strip_receiver: bool,
    ) -> Option<()> {
        let tparams = tparams
            .into_iter()
            .flat_map(|tparams| tparams.iter())
            .collect::<Vec<_>>();
        // Type parameters are declared with PEP 695 syntax only on Python 3.12+, when none has a
        // default and all were written in source. Otherwise every type parameter becomes its
        // gradual form, which keeps the stub a compatible override.
        let declare = self.python_version >= PythonVersion::new(3, 12, 0)
            && tparams.iter().all(|param| {
                param.default().is_none()
                    && matches!(
                        param.identity().origin,
                        QuantifiedOrigin::Pep695 | QuantifiedOrigin::ScopedLegacy
                    )
            });
        let erased;
        let callable = if declare || tparams.is_empty() {
            callable
        } else {
            erased = callable.with_gradual_tparams(tparams.iter().copied());
            &erased
        };
        if declare && !tparams.is_empty() {
            self.text("[");
            for (i, param) in tparams.into_iter().enumerate() {
                if i > 0 {
                    self.text(", ");
                }
                self.text(&param.display_name_with_prefix().to_string());
                match param.restriction() {
                    Restriction::Bound(ty) => {
                        self.text(": ");
                        self.annotation(ty);
                    }
                    Restriction::Constraints(types) => {
                        self.text(": (");
                        for (i, ty) in types.iter().enumerate() {
                            if i > 0 {
                                self.text(", ");
                            }
                            self.annotation(ty);
                        }
                        self.text(")");
                    }
                    Restriction::Unrestricted => {}
                    Restriction::ShapeExtension(_) => return None,
                }
            }
            self.text("]");
        }
        self.text("(");
        let param_spec_params;
        let params = match &callable.params {
            Params::List(params) => params,
            Params::ParamSpec(prefix, param_spec) => {
                // Types do not record the names of the `*args, **kwargs` pair, so `params`
                // names it. Anything but an unsolved `P` becomes the gradual
                // `*args: Any, **kwargs: Any`, which accepts every call.
                let (args, kwargs) = match param_spec {
                    Type::Quantified(q) => (Type::Args(q.clone()), Type::Kwargs(q.clone())),
                    _ => (Type::any_explicit(), Type::any_explicit()),
                };
                param_spec_params = ParamList::new(
                    prefix
                        .iter()
                        .map(PrefixParam::to_param_preserve_name)
                        .chain([Param::Varargs(None, args), Param::Kwargs(None, kwargs)])
                        .collect(),
                );
                &param_spec_params
            }
            _ => return None,
        };
        self.params(params, strip_receiver);
        self.text(") -> ");
        self.annotation(&callable.ret);
        Some(())
    }

    fn params(&mut self, params: &ParamList, strip_receiver: bool) {
        let mut names = params
            .items()
            .iter()
            .filter_map(|param| Some(param.name()?.as_str().to_owned()))
            .collect::<HashSet<_>>();
        let mut posonly = false;
        let mut kwonly = false;
        for (i, param) in params.items().iter().enumerate() {
            if i > 0 {
                self.text(", ");
            }
            if matches!(param, Param::PosOnly(..)) {
                posonly = true;
            } else if posonly {
                self.text("/, ");
                posonly = false;
            }
            if matches!(param, Param::Varargs(..)) {
                kwonly = true;
                self.text("*");
            } else if matches!(param, Param::Kwargs(..)) {
                self.text("**");
            } else if matches!(param, Param::KwOnly(..)) && !kwonly {
                kwonly = true;
                self.text("*, ");
            }
            if let Some(name) = param.name() {
                self.text(name.as_str());
            } else {
                // An unnamed parameter gets a conventional name that no other parameter uses.
                let base = match param {
                    Param::Varargs(..) => "args".to_owned(),
                    Param::Kwargs(..) => "kwargs".to_owned(),
                    _ => format!("arg{i}"),
                };
                let name = unique_name(&base, |name| names.contains(name));
                self.text(&name);
                names.insert(name);
            }
            if !(strip_receiver && i == 0 && matches!(param, Param::PosOnly(..) | Param::Pos(..))) {
                self.text(": ");
                self.annotation(param.as_type());
            }
            if let Param::PosOnly(_, _, Required::Optional(default))
            | Param::Pos(_, _, Required::Optional(default))
            | Param::KwOnly(_, _, Required::Optional(default)) = param
            {
                self.text(" = ");
                match default.as_ref() {
                    Some(DefaultValue {
                        display: Some(text),
                        ..
                    }) => self.text(text),
                    Some(DefaultValue { ty: Type::None, .. }) => self.text("None"),
                    Some(DefaultValue {
                        ty: Type::Literal(literal),
                        ..
                    }) if !matches!(literal.value, Lit::Enum(_)) => {
                        self.text(&literal.value.to_string());
                    }
                    _ => {
                        // Arbitrary default expressions can depend on names in the defining module.
                        self.reference("typing", "cast");
                        self.text("(");
                        self.reference("typing", "Any");
                        self.text(", ...)");
                    }
                }
            }
        }
        if posonly {
            self.text(", /");
        }
    }
}
