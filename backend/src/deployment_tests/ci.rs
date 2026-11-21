//! The CI workflows, Renovate and the PostgreSQL test gate, checked as text.

const CI: &str = include_str!("../../../.github/workflows/ci.yml");
const PUBLISH: &str = include_str!("../../../.github/workflows/docker-publish.yml");
const RETENTION: &str = include_str!("../../../.github/workflows/ghcr-retention.yml");
const RENOVATE: &str = include_str!("../../../renovate.json5");
const PROTECTION: &str = include_str!("../../../scripts/github-protection.sh");

/// A JSON5 config written as JSON plus whole-line `//` comments, parsed as JSON.
fn json_without_comments(text: &str) -> serde_json::Value {
    let json: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    serde_json::from_str(&json).expect("JSON once the comment lines are dropped")
}

/// The text of job `name` in a workflow (up to the next job), one `\n` per line.
fn job(workflow: &str, name: &str) -> String {
    let header = format!("  {name}:");
    let lines: Vec<&str> = workflow
        .lines()
        .skip_while(|l| *l != header)
        .skip(1)
        .take_while(|l| l.is_empty() || l.starts_with("   "))
        .collect();
    assert!(!lines.is_empty(), "job {name}");
    lines.join("\n") + "\n"
}

fn on_main(workflow: &str) -> bool {
    workflow.contains("  push:\n    branches: [main]")
        && workflow.contains("  pull_request:\n    branches: [main]")
}

// @spec DEPLOY-CI-001
#[test]
fn ci_gates_the_backend_with_postgres_and_the_frontend_on_node_26() {
    assert!(on_main(CI));
    let backend = job(CI, "backend");
    for step in [
        "run: cargo fmt --check",
        "run: cargo clippy -- -D warnings",
        "run: cargo test --features db-tests",
        "DATABASE_URL: postgres://",
    ] {
        assert!(backend.contains(step), "backend: {step}");
    }
    // The tag may carry a digest: Renovate pins `postgres:18@sha256:...`.
    assert!(
        backend
            .lines()
            .map(str::trim)
            .any(|l| { l == "image: postgres:18" || l.starts_with("image: postgres:18@sha256:") }),
        "backend: postgres:18 service"
    );
    let frontend = job(CI, "frontend");
    for step in ["node-version: 26", "run: npm ci\n", "run: npm run lint"] {
        assert!(frontend.contains(step), "frontend: {step}");
    }
}

/// Job `name` of `workflow`, which must be a call of the organization's shared
/// workflow `file` pinned to a full commit sha.
fn shared_call(workflow: &str, name: &str, file: &str) -> String {
    let text = job(workflow, name);
    let prefix = format!("uses: chainringlabs/ci/.github/workflows/{file}@");
    let reference = text
        .lines()
        .find_map(|l| l.trim().strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("job {name} calls the shared {file}"));
    let sha = reference.split_whitespace().next().unwrap_or_default();
    assert!(
        sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()),
        "the shared workflow is pinned to a full commit sha"
    );
    text
}

/// The two places the supply-chain scan is called from.
fn scan_calls() -> [String; 2] {
    [
        shared_call(CI, "supply-chain", "supply-chain.yml"),
        shared_call(PUBLISH, "supply-chain", "supply-chain.yml"),
    ]
}

// @spec DEPLOY-CI-002
#[test]
fn every_push_and_pr_runs_the_supply_chain_scan() {
    assert!(on_main(CI));
    // What the scan does lives in the shared workflow; this repository says
    // where its lockfiles are and which Node installs the frontend.
    for call in scan_calls() {
        for input in [
            "npm-dir: frontend",
            "cargo-dir: backend",
            "socket-dir: frontend",
            "node-version: \"26\"",
            "contents: read",
        ] {
            assert!(call.contains(input), "{input}");
        }
    }
}

// @spec DEPLOY-CI-003
#[test]
fn the_socket_secret_reaches_the_scan_without_being_required() {
    // The shared workflow declares the secret optional and skips only its
    // Socket step without it; the callers pass whatever secrets exist.
    for call in scan_calls() {
        assert!(call.contains("secrets: inherit"));
    }
}

// @spec DEPLOY-CI-011
#[test]
fn main_is_scanned_in_full_every_week() {
    assert!(CI.contains("  schedule:\n    - cron: \"0 13 * * 1\""));
    // Only the scan runs on the schedule; the build and test jobs sit it out.
    for name in ["backend", "frontend"] {
        assert!(
            job(CI, name).contains("if: ${{ github.event_name != 'schedule' }}"),
            "{name} is skipped on the schedule"
        );
    }
    assert!(!job(CI, "supply-chain").contains("if:"));
}

/// The `image` job: a call of the organization's shared publish workflow,
/// which holds the build, push, signing and attestation steps for every repo.
fn shared_publish_call() -> String {
    shared_call(PUBLISH, "image", "docker-publish.yml")
}

// @spec DEPLOY-CI-004
#[test]
fn a_pr_builds_the_image_after_the_scan_without_pushing() {
    assert!(PUBLISH.contains("  pull_request:\n    branches: [main]"));
    // The shared workflow builds without pushing on pull requests.
    let image = shared_publish_call();
    assert!(image.contains("needs: supply-chain"));
    shared_call(PUBLISH, "supply-chain", "supply-chain.yml");
    assert!(
        super::DOCKERFILE.contains("RUN npm run build"),
        "the image build includes the frontend build"
    );
}

// @spec DEPLOY-CI-005
#[test]
fn main_and_tags_push_a_signed_attested_image_to_ghcr() {
    assert!(PUBLISH.contains("    branches: [main]\n    tags: [\"v*\"]"));
    // Tagging, pushing, cosign signing and the two attestations are the shared
    // workflow's; this repository grants what they need and nothing wider.
    let image = shared_publish_call();
    for expected in [
        "contents: read",
        "packages: write",
        "id-token: write",
        "attestations: write",
        "artifact-metadata: write",
    ] {
        assert!(image.contains(expected), "{expected}");
    }
    assert!(
        PUBLISH.contains("\npermissions: {}\n"),
        "no permission is granted workflow-wide"
    );
    assert!(
        !PUBLISH.contains("cosign sign") && !PUBLISH.contains("runs-on:"),
        "this repository carries no signing steps of its own"
    );
}

// @spec DEPLOY-CI-006
#[test]
fn retention_keeps_latest_and_five_tagged_versions_weekly() {
    assert!(RETENTION.contains("workflow_dispatch:"));
    assert!(RETENTION.contains("- cron: '0 6 * * 1'"));
    // The pruning itself lives in the organization's shared workflow, which
    // always deletes untagged manifests; this repository states what to keep.
    for expected in [
        "uses: chainringlabs/ci/.github/workflows/ghcr-retention.yml@",
        "keep-n-tagged: 5",
        "exclude-tags: latest",
        "packages: write",
    ] {
        assert!(RETENTION.contains(expected), "{expected}");
    }
}

// @spec DEPLOY-CI-007
#[test]
fn renovate_takes_its_policy_from_the_shared_preset() {
    let config = json_without_comments(RENOVATE);
    assert!(
        RENOVATE.contains("@spec DEPLOY-CI-007"),
        "the config carries its spec"
    );
    let extends: Vec<String> = serde_json::from_value(config["extends"].clone()).unwrap();
    assert_eq!(extends, ["github>chainringlabs/renovate-config"]);
    // Pinning, grouping, the 7-day age and the labels are the preset's. A key
    // added here could loosen them, so the config holds nothing else.
    let mut keys: Vec<&str> = config
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["$schema", "extends"]);
}

// @spec DEPLOY-CI-008
#[test]
fn the_branch_protection_script_states_the_agreed_settings() {
    for setting in [
        // A PR is required, with no approvals (the owner is the only reviewer).
        r#""required_approving_review_count": 0"#,
        r#""strict": true"#,
        // The CI jobs are required: nothing else stops a red auto-merged dependency update.
        r#""context": "Backend (Rust)", "app_id": 15368"#,
        r#""context": "Frontend (React/TypeScript)", "app_id": 15368"#,
        r#""context": "Supply chain / Scan", "app_id": 15368"#,
        r#""context": "image / Build", "app_id": 15368"#,
        // The owner keeps an emergency path; force-pushes and deleting main stay off.
        r#""enforce_admins": false"#,
        r#""allow_force_pushes": false"#,
        r#""allow_deletions": false"#,
        // Merged PR branches are deleted.
        r#""delete_branch_on_merge": true"#,
    ] {
        assert!(PROTECTION.contains(setting), "missing {setting}");
    }
    assert!(PROTECTION.contains("check)") && PROTECTION.contains("apply)"));
    assert!(PROTECTION.contains(r#"REPO="chainringlabs/turfops""#));
    assert!(PROTECTION.contains("repos/$REPO/branches/main/protection"));
    // GitGuardian is the organization's rule: the script checks it is in force and
    // does not repeat it in the repository's own protection.
    assert!(PROTECTION.contains("repos/$REPO/rules/branches/main"));
    assert!(PROTECTION.contains(r#""gitguardian_required": true"#));
    assert!(
        !PROTECTION.contains(r#""context": "GitGuardian Security Checks""#),
        "GitGuardian is not repeated in the repository's protection"
    );
    assert!(
        !PROTECTION.contains("GITHUB_TOKEN="),
        "no token in the script"
    );
}
