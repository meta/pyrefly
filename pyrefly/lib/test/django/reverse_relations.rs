/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use dupe::Dupe;
use pyrefly_build::handle::Handle;
use pyrefly_python::module_name::ModuleName;
use pyrefly_python::module_path::ModulePath;
use pyrefly_util::arc_id::ArcId;
use pyrefly_util::events::CategorizedEvents;
use pyrefly_util::thread_pool::ThreadCount;
use tempfile::TempDir;

use crate::binding::binding::KeyClassSynthesizedFields;
use crate::config::config::ConfigFile;
use crate::config::config::ConfigSource;
use crate::config::error_kind::ErrorKind;
use crate::config::error_kind::Severity;
use crate::config::finder::ConfigFinder;
use crate::django_testcase;
use crate::state::load::FileContents;
use crate::state::require::Require;
use crate::state::state::State;
use crate::test::django::util::django_env;
use crate::test::util::TestEnv;
use crate::test::util::get_class;
use crate::testcase;

// Cross-module reverse relations are only discovered from explicitly configured model modules.
fn django_env_with_separate_models() -> TestEnv {
    let mut env = django_env();
    env.add(
        "author",
        r#"
from django.db import models

class Author(models.Model):
    name = models.CharField(max_length=100)
"#,
    );
    env
}

fn django_env_with_configured_models() -> TestEnv {
    django_env_with_separate_models().with_django_model_modules(&["main"])
}

fn django_env_with_multiple_model_modules() -> TestEnv {
    let mut env = django_env_with_separate_models();
    env.add(
        "magazine",
        r#"
from django.db import models
from .author import Author

class Magazine(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)
"#,
    );
    env.with_django_model_modules(&["main", "magazine"])
}

fn django_env_with_module(name: &str, code: &str) -> TestEnv {
    let mut env = django_env();
    env.add(name, code);
    env
}

fn django_env_without_auto_field() -> TestEnv {
    let mut env = TestEnv::new();
    env.add("django", "");
    env.add("django.db", "");
    env.add(
        "django.db.models",
        r#"
from django.db.models.base import Model
"#,
    );
    env.add("django.db.models.base", "class Model: pass");
    env
}

#[test]
fn test_reverse_relations_incremental() {
    let mut env = django_env().with_django_model_modules(&["relations"]);
    env.add(
        "author",
        "from django.db.models import Model\nclass Author(Model): pass",
    );
    env.add(
        "publisher",
        "from django.db.models import Model\nclass Publisher(Model): pass",
    );
    env.add("relations", "");
    env.add(
        "main",
        r#"
from typing import assert_type
from django.db.models.fields.related_descriptors import RelatedManager
from author import Author
from publisher import Publisher
from relations import Book

assert_type(Author().books, RelatedManager[Book])
assert_type(Publisher().books, RelatedManager[Book])
"#,
    );
    let main = Handle::new(
        ModuleName::from_str("main"),
        ModulePath::memory(PathBuf::from("main.py")),
        env.sys_info(),
    );
    let state = State::new(env.config_finder(), ThreadCount::Inline);
    for (iteration, related_name) in ["books", "works", "+", "books"].into_iter().enumerate() {
        let mut transaction = state.new_committable_transaction(Require::Exports, None);
        if iteration == 0 {
            transaction.as_mut().set_memory(env.get_memory());
        }
        transaction.as_mut().set_memory(vec![(
            PathBuf::from("relations.py"),
            Some(Arc::new(FileContents::from_source(format!(
                r#"
from django.db import models
from author import Author
from publisher import Publisher

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name="{related_name}")
    publisher = models.ForeignKey(Publisher, on_delete=models.CASCADE, related_name="{related_name}")
"#
            )))),
        )]);
        // Only the consumer is checked eagerly. Both target models are solved on demand.
        transaction
            .as_mut()
            .run(&[main.dupe()], Require::Everything, None);
        let errors = transaction
            .as_ref()
            .get_errors([&main])
            .collect_display_errors();
        assert_eq!(
            errors.len(),
            if related_name == "books" { 0 } else { 4 },
            "{errors:?}"
        );
        for error in &errors {
            match error.error_kind() {
                ErrorKind::MissingAttribute => {
                    assert!(
                        error.msg().contains("has no attribute `books`"),
                        "{error:?}"
                    );
                }
                ErrorKind::AssertType => {
                    assert!(error.msg().contains("assert_type(Unknown,"), "{error:?}");
                }
                _ => panic!("unexpected diagnostic: {error:?}"),
            }
        }
        state.commit_transaction(transaction, None);
    }
}

#[test]
fn test_reverse_relations_config_list_invalidation() {
    let root = TempDir::new().unwrap();
    let config_path = root.path().join(ConfigFile::PYREFLY_FILE_NAME);
    let main_path = root.path().join("main.py");
    fs::write(&main_path, "from author import Author\nAuthor().book_set\n").unwrap();
    fs::write(
        root.path().join("author.py"),
        "from django.db.models import Model\nclass Author(Model): pass\n",
    )
    .unwrap();
    fs::write(
        root.path().join("book.py"),
        r#"
from django.db import models
from author import Author

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)
"#,
    )
    .unwrap();

    let env = django_env();
    let mut config = ConfigFile {
        python_environment: env.config().python_environment.clone(),
        search_path_from_file: vec![root.path().to_path_buf()],
        ..Default::default()
    };
    config.interpreters.skip_interpreter_query = true;
    let main = Handle::new(
        ModuleName::from_str("main"),
        ModulePath::filesystem(main_path),
        env.sys_info(),
    );
    // Keep the Django stubs in the test environment, independent of repository configs.
    let stub_config = env.config();
    let fallback = stub_config.dupe();
    let project_root = root.path().to_path_buf();
    let finder = ConfigFinder::new_custom(
        Box::new(move |_, path| {
            Ok((!path.as_path().starts_with(&project_root)).then(|| stub_config.dupe()))
        }),
        Box::new(|path| {
            let (mut config, mut errors) = ConfigFile::from_file(path);
            errors.extend(config.configure());
            (ArcId::new(config), errors)
        }),
        Box::new(move |_, _| fallback.dupe()),
        Box::new(|config, _| config),
        Box::new(|| {}),
    );
    let state = State::new(finder, ThreadCount::Inline);
    for (iteration, enabled) in [false, true, false, true].into_iter().enumerate() {
        config.django_model_modules = if enabled {
            vec![ModuleName::from_str("book")]
        } else {
            Vec::new()
        };
        fs::write(&config_path, toml::to_string(&config).unwrap()).unwrap();
        let (_, errors) = ConfigFile::from_file(&config_path);
        assert!(
            errors.is_empty(),
            "{:?}",
            errors
                .iter()
                .map(|error| error.get_message())
                .collect::<Vec<_>>()
        );
        let mut transaction = state.new_committable_transaction(Require::Exports, None);
        if iteration != 0 {
            transaction.as_mut().invalidate_events(&CategorizedEvents {
                modified: vec![config_path.clone()],
                ..Default::default()
            });
        }
        // The Python files do not change, and only the consumer is checked eagerly.
        transaction
            .as_mut()
            .run(&[main.dupe()], Require::Everything, None);
        let applied = transaction.as_ref().get_config(&main).unwrap();
        assert_eq!(applied.source, ConfigSource::File(config_path.clone()));
        assert_eq!(applied.django_model_modules, config.django_model_modules);
        assert_eq!(
            applied
                .errors(main.path())
                .severity(ErrorKind::MissingAttribute),
            Severity::Error
        );
        let errors = transaction
            .as_ref()
            .get_errors([&main])
            .collect_display_errors();
        assert_eq!(errors.len(), usize::from(!enabled), "{errors:?}");
        if let Some(error) = errors.first() {
            assert_eq!(error.error_kind(), ErrorKind::MissingAttribute);
            assert!(error.msg().contains("has no attribute `book_set`"));
        }
        let config_errors = transaction.as_ref().get_config_errors();
        assert!(
            config_errors.is_empty(),
            "{:?}",
            config_errors
                .iter()
                .map(|error| error.get_message())
                .collect::<Vec<_>>()
        );
        state.commit_transaction(transaction, None);
    }
}

#[test]
fn test_reverse_relations_per_module_config() {
    let mut env = django_env_with_separate_models().with_django_model_modules(&["books"]);
    env.add(
        "other",
        "from django.db.models import Model\nclass Other(Model): pass",
    );
    env.add(
        "books",
        r#"
from django.db import models
from author import Author

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)
"#,
    );
    env.add(
        "magazines",
        r#"
from django.db import models
from other import Other

class Magazine(models.Model):
    other = models.ForeignKey(Other, on_delete=models.CASCADE)
"#,
    );
    env.add(
        "main",
        r#"
from typing import assert_type
from django.db.models.fields.related_descriptors import RelatedManager
from author import Author
from other import Other
from books import Book
from magazines import Magazine

assert_type(Author().book_set, RelatedManager[Book])
assert_type(Other().magazine_set, RelatedManager[Magazine])
"#,
    );
    let config = env.config();
    let mut other_config = (*config).clone();
    other_config.django_model_modules = vec![ModuleName::from_str("magazines")];
    let other_config = ArcId::new(other_config);
    let fallback = config.dupe();
    let finder = ConfigFinder::new_custom(
        Box::new(move |module, _| {
            Ok(Some(if module.as_str() == "other" {
                other_config.dupe()
            } else {
                config.dupe()
            }))
        }),
        Box::new(|_| unreachable!("all modules have an explicit test config")),
        Box::new(move |_, _| fallback.dupe()),
        Box::new(|config, _| config),
        Box::new(|| {}),
    );
    let main = Handle::new(
        ModuleName::from_str("main"),
        ModulePath::memory(PathBuf::from("main.py")),
        env.sys_info(),
    );
    let state = State::new(finder, ThreadCount::Inline);
    let mut transaction = state.new_transaction(Require::Exports, None);
    transaction.set_memory(env.get_memory());
    transaction.run(&[main.dupe()], Require::Everything, None);
    transaction
        .get_errors([&main])
        .check_against_expectations()
        .unwrap();
}

#[test]
fn test_reverse_relations_missing_configured_module() {
    let mut env = django_env_with_separate_models()
        .with_django_model_modules(&["author", "misspelled_models"]);
    env.add("main", "from author import Author\nAuthor()\n");
    let (state, _) = env.to_state();
    let errors = state.transaction().get_config_errors();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].severity(), Severity::Warn);
    assert!(errors[0].get_message().contains("misspelled_models"));
    assert!(errors[0].get_message().contains("django-model-modules"));
}

testcase!(
    test_missing_auto_field_does_not_panic,
    django_env_without_auto_field(),
    r#"
from django.db import models

class Author(models.Model):
    pass

Author()
"#,
);

django_testcase!(
    test_foreign_key_reverse_default_name,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type

class Reporter(models.Model):
    full_name = models.CharField(max_length=70)

class Article(models.Model):
    reporter = models.ForeignKey(Reporter, on_delete=models.CASCADE)

reporter = Reporter()
# Default reverse name is <model_lowercase>_set
assert_type(reporter.article_set, RelatedManager[Article])
"#,
);

django_testcase!(
    test_foreign_key_reverse_custom_name,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type

class Author(models.Model):
    name = models.CharField(max_length=100)

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='written_books')

author = Author()
# Custom related_name should be used instead of default
assert_type(author.written_books, RelatedManager[Book])
"#,
);

django_testcase!(
    test_foreign_key_reverse_disabled,
    r#"
from django.db import models

class Author(models.Model):
    name = models.CharField(max_length=100)

class Book(models.Model):
    # related_name='+' disables the reverse accessor entirely
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='+')

author = Author()
# No reverse accessor should exist
author.book_set  # E: `Author` has no attribute `book_set`
"#,
);

// An attribute whose name is not an identifier is unreachable from Python code, so the
// only way to observe one is to look at the synthesized fields directly.
#[test]
fn test_foreign_key_reverse_invalid_identifier_synthesizes_nothing() {
    let mut env = django_env();
    env.add(
        "main",
        r#"
from django.db import models

class Author(models.Model): ...

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='written_books')

class Magazine(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='123articles')
"#,
    );
    let (state, handle_for) = env.to_state();
    let handle = handle_for("main");
    let reader = state.reader();
    let author = get_class("Author", &handle, &reader);
    let solutions = reader.get_solutions(&handle).unwrap();
    let fields = solutions.get(&KeyClassSynthesizedFields(author.index()));
    let mut names = fields
        .fields()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    names.sort();
    // `123articles` is dropped entirely rather than synthesized under an unusable name.
    assert_eq!(names, vec!["id", "pk", "written_books"]);
}

// A related name that is not a valid identifier names an attribute that can never be
// accessed, so no reverse relation is created at all -- not even under the default name.
django_testcase!(
    test_foreign_key_reverse_invalid_identifier,
    r#"
from django.db import models

class Author(models.Model):
    name = models.CharField(max_length=100)

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name=' written_books ')

class Magazine(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='123articles')

class Poem(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE, related_name='written.poems')

author = Author()
author.written_books  # E: `Author` has no attribute `written_books`
author.book_set  # E: `Author` has no attribute `book_set`
author.magazine_set  # E: `Author` has no attribute `magazine_set`
author.poem_set  # E: `Author` has no attribute `poem_set`
"#,
);

django_testcase!(
    test_foreign_key_reverse_unknown_app_label,
    r#"
from django.db import models

class Author(models.Model): ...

class Book(models.Model):
    author = models.ForeignKey(
        Author,
        on_delete=models.CASCADE,
        related_name='%(app_label)s_books',
    )

author = Author()
author.main_books  # E: `Author` has no attribute `main_books`
"#,
);

testcase!(
    test_foreign_key_reverse_app_label,
    django_env_with_module(
        "myapp.models",
        r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type

class Author(models.Model): ...

class Book(models.Model):
    author = models.ForeignKey(
        Author,
        on_delete=models.CASCADE,
        related_name='%(app_label)s_%(class)s_books',
    )

author = Author()
assert_type(author.myapp_book_books, RelatedManager[Book])
"#
    ),
    "",
);

// Self-referential FK creates reverse accessor on the same model
django_testcase!(
    test_foreign_key_reverse_self_reference,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type

class Person(models.Model):
    name = models.CharField(max_length=100)
    # Self-referential FK: a person can have a parent who is also a Person
    parent = models.ForeignKey('self', null=True, on_delete=models.CASCADE)

person = Person()
assert_type(person.person_set, RelatedManager[Person])
"#,
);

django_testcase!(
    test_foreign_key_reverse_unicode_default_name,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type

class Reporter(models.Model): ...

class ÜberBook(models.Model):
    reporter = models.ForeignKey(Reporter, on_delete=models.CASCADE)

reporter = Reporter()
assert_type(reporter.überbook_set, RelatedManager[ÜberBook])
"#,
);

testcase!(
    test_foreign_key_reverse_cross_module_without_config,
    django_env_with_separate_models(),
    r#"
from django.db import models
from .author import Author

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)

# No model modules are configured, so the reverse relation is not discovered.
author = Author()
author.book_set  # E: `Author` has no attribute `book_set`
"#,
);

testcase!(
    test_foreign_key_reverse_cross_module_configured,
    django_env_with_configured_models(),
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type
from .author import Author

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)

author = Author()
assert_type(author.book_set, RelatedManager[Book])
"#,
);

testcase!(
    test_foreign_key_reverse_multiple_configured_modules,
    django_env_with_multiple_model_modules(),
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import RelatedManager
from typing import assert_type
from .author import Author
from .magazine import Magazine

class Book(models.Model):
    author = models.ForeignKey(Author, on_delete=models.CASCADE)

author = Author()
assert_type(author.book_set, RelatedManager[Book])
assert_type(author.magazine_set, RelatedManager[Magazine])
"#,
);

// OneToOneField reverse relation: returns single object (not a manager like FK)
// Default name is just the lowercase model name without `_set`
django_testcase!(
    test_one_to_one_reverse_default_name,
    r#"
from django.db import models
from typing import assert_type

class Place(models.Model):
    name = models.CharField(max_length=50)

class Restaurant(models.Model):
    place = models.OneToOneField(Place, on_delete=models.CASCADE)

place = Place()
# OneToOne reverse is just the lowercase model name (no _set suffix)
assert_type(place.restaurant, Restaurant)
"#,
);

// ManyToManyField reverse relation: returns a manager like FK
django_testcase!(
    test_many_to_many_reverse_default_name,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager
from typing import assert_type

class Tag(models.Model):
    name = models.CharField(max_length=50)

class Article(models.Model):
    tags = models.ManyToManyField(Tag)

tag = Tag()
# ManyToMany default reverse name is <model_lowercase>_set
assert_type(tag.article_set, ManyRelatedManager[Article, models.Model])
"#,
);

django_testcase!(
    test_one_to_one_reverse_custom_name,
    r#"
from django.db import models
from typing import assert_type

class Place(models.Model):
    name = models.CharField(max_length=50)

class Restaurant(models.Model):
    place = models.OneToOneField(Place, on_delete=models.CASCADE, related_name='dining_spot')

place = Place()
assert_type(place.dining_spot, Restaurant)
"#,
);

django_testcase!(
    test_one_to_one_reverse_disabled,
    r#"
from django.db import models

class Place(models.Model):
    name = models.CharField(max_length=50)

class Restaurant(models.Model):
    place = models.OneToOneField(Place, on_delete=models.CASCADE, related_name='+')

place = Place()
# No reverse accessor should exist
place.restaurant  # E: `Place` has no attribute `restaurant`
"#,
);

django_testcase!(
    test_many_to_many_reverse_custom_name,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager
from typing import assert_type

class Tag(models.Model):
    name = models.CharField(max_length=50)

class Article(models.Model):
    tags = models.ManyToManyField(Tag, related_name='tagged_articles')

tag = Tag()
assert_type(tag.tagged_articles, ManyRelatedManager[Article, models.Model])
"#,
);

django_testcase!(
    test_many_to_many_reverse_disabled,
    r#"
from django.db import models

class Tag(models.Model):
    name = models.CharField(max_length=50)

class Article(models.Model):
    tags = models.ManyToManyField(Tag, related_name='+')

tag = Tag()
# No reverse accessor should exist
tag.article_set  # E: `Tag` has no attribute `article_set`
"#,
);

// Self-referential ManyToMany is symmetrical by default, meaning no reverse accessor
// is created because the relation is bidirectional through the same field
django_testcase!(
    test_many_to_many_self_reference_symmetrical,
    r#"
from django.db import models

class Person(models.Model):
    name = models.CharField(max_length=100)
    # Symmetrical M2M: friends is accessible from both sides via the same field
    friends = models.ManyToManyField('self')

person = Person()
# No person_set because symmetrical=True (default for self-referential M2M)
person.person_set  # E: `Person` has no attribute `person_set`
"#,
);

django_testcase!(
    test_many_to_many_self_reference_asymmetrical,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager
from typing import assert_type

class Person(models.Model):
    name = models.CharField(max_length=100)
    # Asymmetrical M2M: followers vs following relationship
    following = models.ManyToManyField('self', symmetrical=False, related_name='followers')

person = Person()
# With symmetrical=False, reverse accessor is created
assert_type(person.followers, ManyRelatedManager[Person, models.Model])
"#,
);

django_testcase!(
    test_many_to_many_self_reference_dynamic_symmetry,
    r#"
from django.db import models
from django.db.models.fields.related_descriptors import ManyRelatedManager
from typing import assert_type

symmetrical = False

class Person(models.Model):
    following = models.ManyToManyField('self', symmetrical=symmetrical, related_name='followers')

person = Person()
assert_type(person.followers, ManyRelatedManager[Person, models.Model])
"#,
);
