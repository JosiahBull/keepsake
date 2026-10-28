mod support;

#[test]
fn starting_the_server_empties_tmp() {
    let directory = tempfile::tempdir().unwrap();
    let tmp = directory.path().join("tmp");
    std::fs::create_dir_all(tmp.join("nested")).unwrap();
    std::fs::write(tmp.join("interrupted.upload"), b"half a file").unwrap();
    let database = format!("sqlite:{}", directory.path().join("keepsake.db").display());
    let mut server = support::Server::spawn(&[("DATABASE_URL", database.as_str())]);
    server.wait_for_log("keepsake is listening");
    assert!(
        server.output().contains("tmp_removed=2"),
        "{}",
        server.output()
    );
    assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0);
}
