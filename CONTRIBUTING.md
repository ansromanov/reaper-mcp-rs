# Contributing

Thanks for helping. Bug reports, feature requests and pull requests are welcome.

- **Bugs and ideas:** open an [issue](https://github.com/ansromanov/reaper-mcp-rs/issues/new/choose).
  For a feature, describe the REAPER task an agent should be able to do.
- **Security problems:** report them privately, see [SECURITY.md](SECURITY.md).
- **Code:** read [Development](https://ansromanov.github.io/reaper-mcp-rs/development.html) for the
  layout, the tests and how to add a tool. In short:

  ```sh
  cargo fmt --all
  cargo clippy --all-targets -- -D warnings
  cargo test --all-targets   # install lua5.4 to run the bridge tests
  ```

  Use a [Conventional Commit](https://www.conventionalcommits.org) title for the pull request
  (`feat: split items at a position`); it goes into the changelog. Every change goes through a
  pull request, and CI must pass on Linux, macOS and Windows.

By contributing you agree that your contribution is licensed under the MIT License.
