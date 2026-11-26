use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use doto_core::{events::EventBus, paths::AppPaths, secrets::MemoryStore, Db};
use doto_skills::{SkillError, SkillService};

fn write(p: &Path, s: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, s).unwrap();
}

fn service(data: &Path) -> Arc<SkillService> {
    let paths = AppPaths::resolve(Some(data)).unwrap();
    let db = Db::open(&paths.db_path()).unwrap();
    SkillService::new(db, paths, EventBus::default(), Arc::new(MemoryStore::default()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn starts_empty_and_manages_many_folders_with_duplicates_and_install() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    std::env::set_var("DOTO_PROVIDER_HOME", &home);
    let svc = service(&tmp.path().join("data"));
    assert!(svc.list_sources().await.unwrap().is_empty(), "không có nguồn mặc định");
    assert!(svc.list_skills(None, None).await.unwrap().is_empty(), "0 skills ban đầu");

    let f1 = tmp.path().join("f1");
    let f2 = tmp.path().join("f2");
    write(&f1.join("video/SKILL.md"), "---\nname: Video Edit\ndescription: Cut videos\n---\n");
    write(&f1.join("nested/deep/pdf/SKILL.md"), "---\nname: PDF\ndescription: pdf tools\n---\n");
    write(&f2.join("video-edit/SKILL.md"), "---\nname: video edit\ndescription: same id other source\n---\n");
    write(&f2.join("runner/SKILL.md"), "---\nname: Runner\ndescription: has script\n---\n");
    write(&f2.join("runner/run.ps1"), "Write-Host hi");

    let s1 = svc.add_folder(&f1.to_string_lossy(), false).await.unwrap();
    let s2 = svc.add_folder(&f2.to_string_lossy(), false).await.unwrap();
    assert_eq!(s1.skill_count, 2);
    assert_eq!(s2.skill_count, 2);
    assert!(matches!(svc.add_folder(&f1.to_string_lossy(), false).await, Err(SkillError::Conflict(_))));
    assert!(matches!(svc.add_folder("relative/path", false).await, Err(SkillError::Validation(_))));

    let skills = svc.list_skills(None, None).await.unwrap();
    assert_eq!(skills.len(), 4);
    let v1 = skills.iter().find(|s| s.source_id == s1.id && s.canonical_id == "video-edit").unwrap();
    assert_eq!(v1.duplicate_of.len(), 1, "phát hiện trùng canonical id giữa nguồn");
    assert_eq!(svc.list_skills(Some("pdf".into()), None).await.unwrap().len(), 1);

    // tạm dừng nguồn → skill ẩn
    svc.set_flags(&s2.id, Some(false), None, None).await.unwrap();
    assert_eq!(svc.list_skills(None, None).await.unwrap().len(), 2);
    svc.set_flags(&s2.id, Some(true), None, None).await.unwrap();

    // detail + install
    let d = svc.detail(&v1.id).await.unwrap();
    assert!(d.content.contains("Cut videos"));
    assert!(svc.install(&v1.id, "claude", false, false).await.is_err(), "cần confirm");
    let target = svc.install(&v1.id, "claude", true, false).await.unwrap();
    assert!(Path::new(&target).join("SKILL.md").is_file());
    assert!(target.contains(".claude"));
    svc.install(&v1.id, "codex", true, false).await.unwrap();
    assert!(home.join(".agents/skills/video-edit/SKILL.md").is_file());
    let runner = svc.list_skills(Some("Runner".into()), None).await.unwrap().remove(0);
    assert!(runner.has_scripts);
    assert!(svc.install(&runner.id, "antigravity", true, false).await.is_err(), "script cần cho phép rõ ràng");
    svc.install(&runner.id, "antigravity", true, true).await.unwrap();
    assert!(home.join(".gemini/config/skills/runner/run.ps1").is_file());
    let listed = svc.list_skills(Some("Video".into()), Some(s1.id.clone())).await.unwrap();
    assert_eq!(listed[0].installed_providers.len(), 2);
    svc.uninstall(&v1.id, "claude").await.unwrap();
    assert!(!Path::new(&target).exists());

    // rescan phát hiện xoá/sửa
    std::fs::remove_dir_all(f1.join("nested")).unwrap();
    let s1b = svc.rescan(&s1.id).await.unwrap();
    assert_eq!(s1b.skill_count, 1);

    // xoá nguồn → skill biến mất
    svc.remove(&s2.id).await.unwrap();
    assert_eq!(svc.list_skills(None, None).await.unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn watcher_picks_up_new_skill() {
    let tmp = tempfile::tempdir().unwrap();
    let svc = service(&tmp.path().join("data"));
    let f = tmp.path().join("watched");
    std::fs::create_dir_all(&f).unwrap();
    let s = svc.add_folder(&f.to_string_lossy(), true).await.unwrap();
    assert_eq!(s.skill_count, 0);
    write(&f.join("new/SKILL.md"), "---\nname: New\ndescription: d\n---\n");
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        if svc.get_source(&s.id).await.unwrap().skill_count == 1 {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "watcher không phát hiện skill mới");
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Cần mạng: `cargo test -p doto-skills --test service -- --ignored`
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore]
async fn imports_public_github_repo_subdir() {
    let tmp = tempfile::tempdir().unwrap();
    let svc = service(&tmp.path().join("data"));
    let src = svc.add_github("https://github.com/anthropics/skills/tree/main/skills", None).await.unwrap();
    assert!(src.skill_count > 0, "{src:?}");
    assert_eq!(src.current_sha.as_ref().unwrap().len(), 40);
    assert!(!src.private);
    let up = svc.check_update(&src.id).await.unwrap();
    assert!(up.up_to_date);
    assert!(svc.add_github("https://github.com/anthropics/skills/tree/main/skills", None).await.is_err(), "trùng nguồn");
}
