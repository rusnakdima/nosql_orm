#![cfg(feature = "inheritance")]

use nosql_orm::entity::{Entity, EntityMeta};
use nosql_orm::error::OrmResult;
use nosql_orm::inheritance::{
  Discriminator, DiscriminatorValue, Inheritance, InheritanceType,
};
use nosql_orm::provider::Storage;
use nosql_orm::providers::json::JsonProvider;
use nosql_orm::repository::Repository;
use nosql_orm::validators::Validate;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InhPerson {
  pub id: Option<String>,
  pub name: String,
  #[serde(rename = "type")]
  pub kind: Option<String>,
}

impl Entity for InhPerson {
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }

  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }

  fn meta() -> EntityMeta {
    EntityMeta::new("people")
  }
}

impl Validate for InhPerson {
  fn validate(&self) -> OrmResult<()> {
    Ok(())
  }
}

nosql_orm::entity_inheritance!(parent, InhPerson);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InhStudent {
  pub id: Option<String>,
  pub name: String,
  pub grade: String,
  #[serde(rename = "type")]
  pub kind: String,
}

impl Entity for InhStudent {
  fn get_id(&self) -> Option<String> {
    self.id.clone()
  }

  fn set_id(&mut self, id: String) {
    self.id = Some(id);
  }

  fn meta() -> EntityMeta {
    // Single-table inheritance: the child shares the parent's table.
    EntityMeta::new("people")
  }
}

impl Validate for InhStudent {
  fn validate(&self) -> OrmResult<()> {
    Ok(())
  }
}

nosql_orm::entity_inheritance!(sti, InhPerson, InhStudent, "student");

struct InheritanceFixture {
  provider: JsonProvider,
  person_repo: Repository<InhPerson, JsonProvider>,
  student_repo: Repository<InhStudent, JsonProvider>,
  _dir: tempfile::TempDir,
}

async fn fixture() -> InheritanceFixture {
  let dir = tempfile::TempDir::new().unwrap();
  let provider = JsonProvider::new(dir.path()).await.unwrap();
  let person_repo = Repository::new(provider.clone());
  let student_repo = Repository::new(provider.clone());
  InheritanceFixture {
    provider,
    person_repo,
    student_repo,
    _dir: dir,
  }
}

fn student(name: &str, grade: &str) -> InhStudent {
  InhStudent {
    id: None,
    name: name.to_string(),
    grade: grade.to_string(),
    kind: "student".to_string(),
  }
}

#[tokio::test]
async fn sti_queries_are_scoped_by_discriminator() {
  let f = fixture().await;
  f.person_repo
    .insert(InhPerson {
      id: None,
      name: "Carol".to_string(),
      kind: Some("person".to_string()),
    })
    .await
    .unwrap();
  f.student_repo.insert(student("Dave", "A")).await.unwrap();
  f.student_repo.insert(student("Eve", "B")).await.unwrap();

  let students = f.student_repo.query_scoped().find().await.unwrap();
  assert_eq!(students.len(), 2);

  let count = f.student_repo.query_scoped().count().await.unwrap();
  assert_eq!(count, 2);

  // The parent read on the shared table sees every row.
  let everyone = f.person_repo.find_all_scoped().await.unwrap();
  assert_eq!(everyone.len(), 3);
}

#[tokio::test]
async fn find_by_id_scoped_rejects_wrong_type_rows() {
  let f = fixture().await;
  let person = f
    .person_repo
    .insert(InhPerson {
      id: None,
      name: "Carol".to_string(),
      kind: Some("person".to_string()),
    })
    .await
    .unwrap();
  let stu = f.student_repo.insert(student("Dave", "A")).await.unwrap();

  let person_id = person.id.unwrap();
  let student_id = stu.id.unwrap();

  let found = f
    .student_repo
    .find_by_id_scoped(&student_id)
    .await
    .unwrap();
  assert!(found.is_some());

  // Same id space, wrong discriminator: the child must not see the parent row.
  let wrong = f.student_repo.find_by_id_scoped(&person_id).await.unwrap();
  assert!(wrong.is_none());
}

#[tokio::test]
async fn parent_table_reads_use_the_explicit_table() {
  let f = fixture().await;
  f.student_repo.insert(student("Dave", "A")).await.unwrap();

  f.provider
    .insert(
      "people_archive",
      json!({
        "id": "arc-1",
        "name": "Old Student",
        "grade": "A",
        "type": "student"
      }),
    )
    .await
    .unwrap();
  f.provider
    .insert(
      "people_archive",
      json!({
        "id": "arc-2",
        "name": "Old Teacher",
        "grade": "C",
        "type": "teacher"
      }),
    )
    .await
    .unwrap();

  // The child reads a different table, still scoped by its discriminator.
  let archived = f
    .student_repo
    .find_all_in_parent("people_archive")
    .await
    .unwrap();
  assert_eq!(archived.len(), 1);
  assert_eq!(archived[0].name, "Old Student");

  // The parent read of the same table is unscoped.
  let all = f
    .person_repo
    .find_all_in_parent("people_archive")
    .await
    .unwrap();
  assert_eq!(all.len(), 2);
}

#[tokio::test]
async fn parent_table_reads_group_rows_by_entity_type() {
  let f = fixture().await;
  f.provider
    .insert(
      "people_archive",
      json!({
        "id": "arc-1",
        "name": "Old Student",
        "grade": "A",
        "type": "student"
      }),
    )
    .await
    .unwrap();
  f.provider
    .insert(
      "people_archive",
      json!({
        "id": "arc-2",
        "name": "Old Teacher",
        "grade": "C",
        "type": "teacher"
      }),
    )
    .await
    .unwrap();

  let discriminator = Discriminator::new("type")
    .add_value("student", "InhStudent")
    .add_value("teacher", "Teacher");
  assert_eq!(
    discriminator.get_entity_type("student"),
    Some("InhStudent")
  );

  let groups = f
    .student_repo
    .find_all_grouped_in_parent("people_archive", &discriminator)
    .await
    .unwrap();
  assert_eq!(groups.get("InhStudent").map(|v| v.len()), Some(1));
  assert_eq!(groups.get("Teacher").map(|v| v.len()), Some(1));
}

#[tokio::test]
async fn inheritance_metadata_is_reported() {
  assert_eq!(
    InhStudent::inheritance_type(),
    Some(InheritanceType::SingleTable)
  );
  assert_eq!(
    InhStudent::discriminator_value().map(|d| d.as_str().to_string()),
    Some("student".to_string())
  );
  assert_eq!(InhStudent::discriminator_column(), "type");
  assert_eq!(
    InhStudent::parent_entity().map(String::from),
    Some("InhPerson".to_string())
  );
  assert!(InhPerson::discriminator_value().is_none());
  assert_eq!(InhPerson::inheritance_type(), None);

  // DiscriminatorValue keeps its string payload comparable.
  let value = DiscriminatorValue::new("student");
  assert_eq!(value.as_str(), "student");
}
