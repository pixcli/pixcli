# Launch Checklist

## Pre-Launch

### Code Quality
- [ ] `cargo test --workspace` — all tests pass
- [ ] `cargo clippy --workspace -- -D warnings` — no warnings
- [ ] `cargo fmt --all -- --check` — properly formatted
- [ ] `cargo doc --workspace --no-deps` — documentation builds
- [ ] No `todo!()`, `unimplemented!()`, or `dbg!()` in production code
- [ ] All public items have doc comments

### Security
- [ ] No credentials, API keys, or secrets in code
- [ ] .gitignore covers *.p12, *.pem, .env, .pixcli/
- [ ] Credential storage uses proper file permissions (600)
- [ ] Audit logging works correctly

### Documentation
- [ ] README.md is complete and renders correctly
- [ ] CONTRIBUTING.md is written
- [ ] LICENSE is present (MIT)
- [ ] All CLI commands have --help text
- [ ] MCP tool descriptions are clear
- [ ] CHANGELOG.md is up to date

### Packaging
- [ ] `cargo publish --dry-run -p pix-core` succeeds
- [ ] `cargo publish --dry-run -p pix-brcode` succeeds
- [ ] `cargo publish --dry-run -p pix-provider` succeeds
- [ ] `cargo publish --dry-run -p pix-efi` succeeds
- [ ] `cargo publish --dry-run -p pix-mcp` succeeds
- [ ] `cargo publish --dry-run -p pix-webhook-server` succeeds
- [ ] `cargo publish --dry-run -p pixcli` succeeds
- [ ] GitHub Actions CI passes on main
- [ ] GitHub Actions Release builds all 5 targets

### Demo & Content
- [ ] Asciinema demo recorded and uploaded
- [ ] Blog post drafted
- [ ] Social media posts prepared

## Launch Day

### Publishing (in order)
1. [ ] Tag release: `git tag v0.1.0 && git push --tags`
2. [ ] Wait for GitHub Actions to build + create Release
3. [ ] Download and verify all 5 binary archives
4. [ ] `cargo publish -p pix-core`
5. [ ] Wait 1 min, then `cargo publish -p pix-brcode`
6. [ ] Wait 1 min, then `cargo publish -p pix-provider`
7. [ ] Wait 1 min, then `cargo publish -p pix-efi`
8. [ ] Wait 1 min, then `cargo publish -p pix-mcp`
9. [ ] Wait 1 min, then `cargo publish -p pix-webhook-server`
10. [ ] Wait 1 min, then `cargo publish -p pixcli`
11. [ ] Verify `cargo install pixcli` works
12. [ ] Update Homebrew formula with actual SHA256 hashes

### Announcements
- [ ] Tweet / X post
- [ ] Post on r/rust
- [ ] Post on r/brdev
- [ ] Post on TabNews.com.br
- [ ] Post on Hacker News (Show HN)
- [ ] Post on Dev.to
- [ ] Post on LinkedIn

### Post-Launch (Week 1)
- [ ] Monitor GitHub Issues
- [ ] Respond to community feedback
- [ ] Fix critical bugs (patch release if needed)
- [ ] Update README with any corrections
