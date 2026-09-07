// Activation fixtures own real directories, isolated per test thread.
thread_local! {
    static PROJECT_ROOT: tempfile::TempDir = tempfile::TempDir::new().expect("project fixture root");
}

fn fixture_project(name: &str) -> String {
    PROJECT_ROOT.with(|root| {
        let path = root.path().join(name);
        std::fs::create_dir_all(&path).expect("project fixture");
        path.to_string_lossy().into_owned()
    })
}
