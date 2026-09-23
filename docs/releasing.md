# Releasing orthodag

Release-plz prepares releases from Conventional Commit squash titles. A `fix`,
`feat`, `perf` or `refactor` change opens or updates a release PR; a breaking
`!` change receives the corresponding Cargo SemVer bump. CI and documentation
changes alone do not cut a crate release.

Merging the release PR is the publish step. The `release` job publishes the
crate to crates.io through trusted publishing, and a crates.io version cannot be
taken back, only yanked. The GitHub release it creates is a draft, so the notes
can be read before the tag is announced.

To release:

1. Review and squash-merge the release-plz PR after its required checks pass.
2. Confirm the new version appears on crates.io and the matching `vX.Y.Z` tag
   exists.
3. Inspect the draft GitHub release.
4. Run the **Publish release** workflow with that tag.
