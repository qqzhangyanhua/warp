use futures::executor::block_on;
use super::*;
use crate::persistence::{setup_database, start_writer, ModelEvent};
#[test]
fn service_recalls_verbatim_value_after_writer_restart() {
    block_on(async {
        let tempdir = tempfile::tempdir().unwrap();
        let database_path = tempdir.path().join("warp.sqlite");
        let conn = setup_database(&database_path).unwrap();
        let writer = start_writer(conn, database_path.clone()).unwrap();
        let service = PersonalMemoryService::new(writer.sender.clone());

        service
            .create(CreatePersonalMemoryInput::exact(
                "记住我的 GitHub 帐号是 zyh-work".to_string(),
                "zyh-work".to_string(),
                "GitHub 帐号".to_string(),
            ))
            .await
            .unwrap();
        writer.sender.send(ModelEvent::Terminate).unwrap();
        writer.handle.join().unwrap();

        let conn = setup_database(&database_path).unwrap();
        let writer = start_writer(conn, database_path).unwrap();
        let service = PersonalMemoryService::new(writer.sender.clone());
        let result = service.query("GitHub 帐号").await.unwrap();
        let QueryPersonalMemoryResult::Matches(records) = result else {
            panic!("stored record should match after restart");
        };
        assert_eq!(records[0].value_text, "zyh-work");

        writer.sender.send(ModelEvent::Terminate).unwrap();
        writer.handle.join().unwrap();
    });
}

#[test]
fn service_returns_typed_no_match() {
    block_on(async {
        let tempdir = tempfile::tempdir().unwrap();
        let database_path = tempdir.path().join("warp.sqlite");
        let conn = setup_database(&database_path).unwrap();
        let writer = start_writer(conn, database_path).unwrap();
        let service = PersonalMemoryService::new(writer.sender.clone());

        assert_eq!(
            service.query("NAS address").await.unwrap(),
            QueryPersonalMemoryResult::NoMatch
        );

        writer.sender.send(ModelEvent::Terminate).unwrap();
        writer.handle.join().unwrap();
    });
}

#[test]
fn service_keeps_exact_user_authored_text() {
    block_on(async {
        let tempdir = tempfile::tempdir().unwrap();
        let database_path = tempdir.path().join("warp.sqlite");
        let conn = setup_database(&database_path).unwrap();
        let writer = start_writer(conn, database_path).unwrap();
        let service = PersonalMemoryService::new(writer.sender.clone());
        let fact = "Remember my account exactly: Work_ID-01";

        let result = service
            .create(CreatePersonalMemoryInput::exact(
                fact.to_string(),
                "Work_ID-01".to_string(),
                "account".to_string(),
            ))
            .await
            .unwrap();

        assert_eq!(result.record().fact_text, fact);
        assert_eq!(result.record().value_text, "Work_ID-01");
        writer.sender.send(ModelEvent::Terminate).unwrap();
        writer.handle.join().unwrap();
    });
}
